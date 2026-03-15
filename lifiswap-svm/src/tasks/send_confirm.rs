//! Send and confirm task — broadcasts signed transactions and polls for confirmation.
//!
//! Mirrors the TS SDK's `SolanaStandardWaitForTransactionTask`:
//! sends the signed transaction to **all** RPCs in parallel, polls
//! signature status, and re-sends periodically until confirmed or
//! the blockhash expires. The first RPC to confirm wins (parallel
//! racing, matching the TS SDK's `Promise.any` pattern).

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::execution::status::ActionUpdateParams;
use lifiswap::execution::task::{ExecutionContext, ExecutionTask};
use lifiswap::types::{ExecutionActionStatus, ExecutionActionType, TaskStatus};
use solana_commitment_config::CommitmentConfig;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::signature::Signature;
use solana_sdk::transaction::VersionedTransaction;

use super::get_tx_link;
use crate::rpc::RpcPool;

/// Sends signed Solana transactions and waits for on-chain confirmation.
///
/// Uses `RpcPool` for redundant RPC access. Sends the transaction, then
/// polls signature status with periodic re-sends until confirmed or
/// the blockhash expires.
pub struct SvmSendAndConfirmTask {
    rpc_pool: RpcPool,
    skip_simulation: bool,
    signed_txs: Arc<Mutex<Vec<VersionedTransaction>>>,
}

impl std::fmt::Debug for SvmSendAndConfirmTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SvmSendAndConfirmTask")
            .field("rpc_count", &self.rpc_pool.len())
            .field("skip_simulation", &self.skip_simulation)
            .finish_non_exhaustive()
    }
}

impl SvmSendAndConfirmTask {
    pub(crate) const fn new(
        rpc_pool: RpcPool,
        skip_simulation: bool,
        signed_txs: Arc<Mutex<Vec<VersionedTransaction>>>,
    ) -> Self {
        Self {
            rpc_pool,
            skip_simulation,
            signed_txs,
        }
    }
}

const POLL_INTERVAL: tokio::time::Duration = tokio::time::Duration::from_millis(400);
const RESEND_INTERVAL: tokio::time::Duration = tokio::time::Duration::from_secs(1);

/// Send a transaction and poll for confirmation across **all** RPCs in parallel.
///
/// Each RPC independently sends and re-sends the transaction while polling
/// for signature status. The first RPC to observe confirmation wins, and
/// all other tasks are cancelled. This mirrors the TS SDK's `Promise.any`
/// multi-RPC racing strategy for optimal latency.
///
/// Returns the confirmed [`Signature`] or an error if all RPCs fail / expire.
async fn send_and_confirm(
    rpc_pool: &RpcPool,
    tx: &VersionedTransaction,
    signature: Signature,
) -> Result<Signature> {
    let clients = rpc_pool.clients();

    // Shared cancellation flag: set to true when any RPC confirms
    let cancelled = Arc::new(AtomicBool::new(false));

    let mut set = tokio::task::JoinSet::new();
    for client in clients {
        let client = Arc::clone(client);
        let tx = tx.clone();
        let cancelled = Arc::clone(&cancelled);

        set.spawn(
            async move { send_and_poll_single_rpc(&client, &tx, signature, &cancelled).await },
        );
    }

    let mut last_error: Option<LiFiError> = None;
    while let Some(join_result) = set.join_next().await {
        match join_result {
            Ok(Ok(sig)) => {
                cancelled.store(true, Ordering::Release);
                set.abort_all();
                return Ok(sig);
            }
            Ok(Err(e)) => last_error = Some(e),
            Err(join_err) if !join_err.is_cancelled() => {
                last_error = Some(LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: format!("RPC task panicked: {join_err}"),
                });
            }
            Err(_) => {} // cancelled — expected
        }
    }

    Err(last_error.unwrap_or_else(|| LiFiError::Provider {
        code: LiFiErrorCode::ProviderUnavailable,
        message: "All RPCs failed to confirm transaction".to_owned(),
    }))
}

/// Run send + poll loop against a single RPC, returning on confirmation
/// or when `cancelled` is set by another task.
async fn send_and_poll_single_rpc(
    rpc: &RpcClient,
    tx: &VersionedTransaction,
    signature: Signature,
    cancelled: &AtomicBool,
) -> Result<Signature> {
    let commitment = CommitmentConfig::confirmed();

    // Initial send (best-effort — continue even if it fails)
    let _ = rpc.send_transaction(tx).await;

    // Get block height bounds
    let (_, last_valid_block_height) = rpc
        .get_latest_blockhash_with_commitment(commitment)
        .await
        .map_err(|e| LiFiError::Provider {
        code: LiFiErrorCode::ProviderUnavailable,
        message: format!("Failed to get blockhash info: {e}"),
    })?;

    let mut last_resend = tokio::time::Instant::now();

    loop {
        if cancelled.load(Ordering::Acquire) {
            return Err(LiFiError::Transaction {
                code: LiFiErrorCode::InternalError,
                message: "Cancelled by another RPC".to_owned(),
            });
        }

        // Poll signature status
        if let Ok(statuses) = rpc.get_signature_statuses(&[signature]).await
            && let Some(Some(status)) = statuses.value.first()
            && status.satisfies_commitment(commitment)
        {
            if let Some(ref err) = status.err {
                return Err(LiFiError::Transaction {
                    code: LiFiErrorCode::TransactionFailed,
                    message: format!("Transaction failed on-chain: {err:?}"),
                });
            }
            return Ok(signature);
        }

        // Check block height expiry
        let current_height = rpc.get_block_height().await.unwrap_or(0);
        if current_height > last_valid_block_height {
            return Err(LiFiError::Transaction {
                code: LiFiErrorCode::TransactionExpired,
                message: "Transaction expired: block height exceeded the maximum allowed limit."
                    .to_owned(),
            });
        }

        // Periodic re-send (best-effort, matching TS SDK's 1s interval)
        if last_resend.elapsed() >= RESEND_INTERVAL {
            let _ = rpc.send_transaction(tx).await;
            last_resend = tokio::time::Instant::now();
        }

        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

impl ExecutionTask for SvmSendAndConfirmTask {
    fn should_run<'a>(
        &'a self,
        ctx: &'a ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move { !ctx.has_committed_transaction() })
    }

    #[allow(clippy::excessive_nesting)]
    fn run<'a>(
        &'a self,
        ctx: &'a mut ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = Result<TaskStatus>> + Send + 'a>> {
        Box::pin(async move {
            let action_type = if ctx.is_bridge_execution {
                ExecutionActionType::CrossChain
            } else {
                ExecutionActionType::Swap
            };

            let _action = ctx
                .status_manager
                .find_action(ctx.step, action_type)
                .ok_or(LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "Unable to send transaction. Action not found.".to_owned(),
                })?;

            let signed_txs = {
                let guard = self.signed_txs.lock().expect("signed_txs mutex poisoned");
                guard.clone()
            };

            let tx = signed_txs
                .into_iter()
                .next()
                .ok_or_else(|| LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "No signed transactions available for sending.".to_owned(),
                })?;

            let signature =
                tx.signatures
                    .first()
                    .copied()
                    .ok_or_else(|| LiFiError::Transaction {
                        code: LiFiErrorCode::InternalError,
                        message: "Signed transaction has no signatures.".to_owned(),
                    })?;

            // Optional simulation before sending
            if !self.skip_simulation {
                let sim_result = self
                    .rpc_pool
                    .call_with_retry(|rpc| {
                        let tx_clone = tx.clone();
                        async move {
                            rpc.simulate_transaction(&tx_clone).await.map_err(|e| {
                                LiFiError::Transaction {
                                    code: LiFiErrorCode::TransactionSimulationFailed,
                                    message: format!("Simulation RPC error: {e}"),
                                }
                            })
                        }
                    })
                    .await?;

                if let Some(err) = sim_result.value.err {
                    return Err(LiFiError::Transaction {
                        code: LiFiErrorCode::TransactionSimulationFailed,
                        message: format!("Transaction simulation failed: {err:?}"),
                    });
                }
            }

            // Send and wait for confirmation
            let confirmed_sig = send_and_confirm(&self.rpc_pool, &tx, signature).await?;

            let tx_sig = confirmed_sig.to_string();
            let tx_link = get_tx_link(ctx.from_chain, &tx_sig);

            ctx.status_manager.update_action(
                ctx.step,
                action_type,
                ExecutionActionStatus::Pending,
                Some(ActionUpdateParams {
                    tx_hash: Some(tx_sig),
                    tx_link,
                    ..Default::default()
                }),
            )?;

            if ctx.is_bridge_execution {
                ctx.status_manager.update_action(
                    ctx.step,
                    action_type,
                    ExecutionActionStatus::Done,
                    None,
                )?;
            }

            Ok(TaskStatus::Completed)
        })
    }
}
