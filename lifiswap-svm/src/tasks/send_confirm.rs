//! Send and confirm task — broadcasts signed transactions and polls for confirmation.
//!
//! Mirrors the TS SDK's `SolanaStandardWaitForTransactionTask`:
//! sends the signed transaction via the RPC pool, polls signature status,
//! and re-sends periodically until confirmed or the blockhash expires.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::execution::status::ActionUpdateParams;
use lifiswap::execution::task::{ExecutionContext, ExecutionTask};
use lifiswap::types::{ExecutionActionStatus, ExecutionActionType, TaskStatus};
use solana_commitment_config::CommitmentConfig;
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
    pub(crate) fn new(
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
const RESEND_INTERVAL: tokio::time::Duration = tokio::time::Duration::from_secs(2);

/// Send a transaction and poll for confirmation with periodic re-sends.
///
/// Returns the confirmed [`Signature`] or an error if the blockhash expires.
async fn send_and_confirm(
    rpc_pool: &RpcPool,
    tx: &VersionedTransaction,
    signature: Signature,
) -> Result<Signature> {
    let commitment = CommitmentConfig::confirmed();

    // Initial send
    rpc_pool
        .call_with_retry(|rpc| {
            let tx = tx.clone();
            async move {
                rpc.send_transaction(&tx)
                    .await
                    .map_err(|e| LiFiError::Transaction {
                        code: LiFiErrorCode::TransactionFailed,
                        message: format!("Failed to send transaction: {e}"),
                    })
            }
        })
        .await?;

    // Get the last valid block height for this blockhash
    let (_, last_valid_block_height) = rpc_pool
        .call_with_retry(|rpc| async move {
            rpc.get_latest_blockhash_with_commitment(commitment)
                .await
                .map_err(|e| LiFiError::Provider {
                    code: LiFiErrorCode::ProviderUnavailable,
                    message: format!("Failed to get blockhash info: {e}"),
                })
        })
        .await?;

    let mut last_resend = tokio::time::Instant::now();

    loop {
        // Poll signature status
        let status_result = rpc_pool
            .call_with_retry(|rpc| {
                let sig = signature;
                async move {
                    rpc.get_signature_statuses(&[sig])
                        .await
                        .map_err(|e| LiFiError::Provider {
                            code: LiFiErrorCode::ProviderUnavailable,
                            message: format!("Failed to get signature status: {e}"),
                        })
                }
            })
            .await;

        if let Ok(statuses) = status_result
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
        let current_height = rpc_pool
            .call_with_retry(|rpc| async move {
                rpc.get_block_height()
                    .await
                    .map_err(|e| LiFiError::Provider {
                        code: LiFiErrorCode::ProviderUnavailable,
                        message: format!("Failed to get block height: {e}"),
                    })
            })
            .await
            .unwrap_or(0);

        if current_height > last_valid_block_height {
            return Err(LiFiError::Transaction {
                code: LiFiErrorCode::TransactionExpired,
                message: "Transaction expired: block height exceeded the maximum allowed limit."
                    .to_owned(),
            });
        }

        // Periodic re-send (best-effort)
        if last_resend.elapsed() >= RESEND_INTERVAL {
            let _ = rpc_pool
                .call_with_retry(|rpc| {
                    let tx = tx.clone();
                    async move {
                        rpc.send_transaction(&tx)
                            .await
                            .map_err(|e| LiFiError::Transaction {
                                code: LiFiErrorCode::TransactionFailed,
                                message: format!("Resend failed: {e}"),
                            })
                    }
                })
                .await;
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
