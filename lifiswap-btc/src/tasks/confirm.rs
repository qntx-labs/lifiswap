//! Confirm task — polls for Bitcoin transaction confirmation.
//!
//! Mirrors the TS SDK's `BitcoinWaitForTransactionTask`: polls the
//! transaction status via the blockchain API until confirmed on-chain.
//! Supports RBF (Replace-By-Fee) detection: if the original transaction
//! disappears from the mempool, checks whether its inputs were spent by
//! a replacement transaction and updates the action accordingly.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::execution::status::ActionUpdateParams;
use lifiswap::execution::task::{ExecutionContext, ExecutionTask};
use lifiswap::types::{ExecutionActionStatus, ExecutionActionType, TaskStatus};

use super::{BtcTxInputs, get_tx_link};
use crate::api::BlockchainApi;

/// Polls Bitcoin transaction confirmation status with RBF detection.
///
/// After the sign task broadcasts the transaction, this task polls
/// the blockchain API at 10-second intervals until the transaction
/// is confirmed (included in a block).
///
/// If the original transaction vanishes from the mempool (404), the
/// task checks the first input's outspend to detect RBF replacement.
/// A cancelled replacement raises [`LiFiErrorCode::TransactionCanceled`].
pub struct BtcConfirmTask {
    api: BlockchainApi,
    tx_inputs: Arc<BtcTxInputs>,
}

impl std::fmt::Debug for BtcConfirmTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BtcConfirmTask").finish_non_exhaustive()
    }
}

impl BtcConfirmTask {
    pub(crate) const fn new(api: BlockchainApi, tx_inputs: Arc<BtcTxInputs>) -> Self {
        Self { api, tx_inputs }
    }
}

/// Polling interval: 10 seconds (matching TS SDK's `pollingIntervalMs: 10_000`).
const POLL_INTERVAL: tokio::time::Duration = tokio::time::Duration::from_secs(10);

/// Maximum time to wait for confirmation: 30 minutes.
const CONFIRM_TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_mins(30);

/// Reason why a transaction was replaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplacementReason {
    /// Transaction was sped up (same destination, higher fee).
    SpedUp,
    /// Transaction was cancelled (inputs redirected elsewhere).
    Cancelled,
}

impl ExecutionTask for BtcConfirmTask {
    fn should_run<'a>(
        &'a self,
        ctx: &'a ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            let action_type = if ctx.is_bridge_execution {
                ExecutionActionType::CrossChain
            } else {
                ExecutionActionType::Swap
            };

            ctx.status_manager
                .find_action(ctx.step, action_type)
                .and_then(|a| a.tx_hash.as_ref().map(|_| ()))
                .is_some()
        })
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = Result<TaskStatus>> + Send + 'a>> {
        Box::pin(self.poll_confirmation(ctx))
    }
}

/// Outcome of a single poll iteration.
enum PollOutcome {
    Confirmed,
    Pending,
    Replaced(ReplacementTx),
    NotFound,
}

struct ReplacementTx {
    txid: String,
    reason: ReplacementReason,
}

impl BtcConfirmTask {
    async fn poll_confirmation(&self, ctx: &mut ExecutionContext<'_>) -> Result<TaskStatus> {
        let action_type = if ctx.is_bridge_execution {
            ExecutionActionType::CrossChain
        } else {
            ExecutionActionType::Swap
        };

        let mut current_tx_hash = ctx
            .status_manager
            .find_action(ctx.step, action_type)
            .and_then(|a| a.tx_hash.clone())
            .ok_or(LiFiError::Transaction {
                code: LiFiErrorCode::TransactionUnprepared,
                message: "Transaction hash not set.".to_owned(),
            })?;

        tracing::info!(
            tx_hash = current_tx_hash.as_str(),
            "Waiting for Bitcoin transaction confirmation"
        );

        let deadline = tokio::time::Instant::now() + CONFIRM_TIMEOUT;
        let mut tx_was_seen_in_mempool = false;
        let mut consecutive_not_found = 0u32;

        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err(LiFiError::Transaction {
                    code: LiFiErrorCode::TransactionExpired,
                    message: "Transaction confirmation timed out after 30 minutes.".to_owned(),
                });
            }

            let outcome = self
                .poll_once(
                    &current_tx_hash,
                    tx_was_seen_in_mempool,
                    consecutive_not_found,
                    ctx.from_chain,
                )
                .await;

            match outcome {
                PollOutcome::Confirmed => {
                    if ctx.is_bridge_execution {
                        ctx.status_manager.update_action(
                            ctx.step,
                            action_type,
                            ExecutionActionStatus::Done,
                            None,
                        )?;
                    }
                    return Ok(TaskStatus::Completed);
                }
                PollOutcome::Pending => {
                    tx_was_seen_in_mempool = true;
                    consecutive_not_found = 0;
                }
                PollOutcome::NotFound => {
                    consecutive_not_found += 1;
                }
                PollOutcome::Replaced(replacement) => {
                    if replacement.reason == ReplacementReason::Cancelled {
                        return Err(LiFiError::Transaction {
                            code: LiFiErrorCode::TransactionCanceled,
                            message: "User canceled transaction.".to_owned(),
                        });
                    }

                    tracing::info!(
                        old_tx = current_tx_hash.as_str(),
                        new_tx = replacement.txid.as_str(),
                        "Transaction replaced (RBF speed-up)"
                    );

                    let tx_link = get_tx_link(ctx.from_chain, &replacement.txid);
                    ctx.status_manager.update_action(
                        ctx.step,
                        action_type,
                        ExecutionActionStatus::Pending,
                        Some(ActionUpdateParams {
                            tx_hash: Some(replacement.txid.clone()),
                            tx_link,
                            ..Default::default()
                        }),
                    )?;

                    current_tx_hash = replacement.txid;
                    tx_was_seen_in_mempool = false;
                    consecutive_not_found = 0;
                }
            }

            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }

    /// Run a single status check, returning a flat [`PollOutcome`].
    async fn poll_once(
        &self,
        tx_hash: &str,
        was_seen: bool,
        consecutive_miss: u32,
        from_chain: &lifiswap::types::Chain,
    ) -> PollOutcome {
        match self.api.get_tx_status(tx_hash).await {
            Ok(status) if status.confirmed => {
                tracing::info!(
                    tx_hash,
                    block_height = ?status.block_height,
                    "Bitcoin transaction confirmed"
                );
                PollOutcome::Confirmed
            }
            Ok(_) => {
                tracing::debug!(tx_hash, "Transaction not yet confirmed, polling...");
                PollOutcome::Pending
            }
            Err(e) => {
                tracing::warn!(tx_hash, error = %e, "Failed to check tx status");

                if was_seen
                    && consecutive_miss >= 2
                    && let Some(replacement) = self.detect_replacement(tx_hash, from_chain).await
                {
                    return PollOutcome::Replaced(replacement);
                }
                PollOutcome::NotFound
            }
        }
    }

    /// Detect if the original transaction was replaced via RBF by checking
    /// whether its first input's previous output was spent by a different tx.
    async fn detect_replacement(
        &self,
        original_txid: &str,
        from_chain: &lifiswap::types::Chain,
    ) -> Option<ReplacementTx> {
        let (prev_txid, prev_vout) = {
            let guard = self.tx_inputs.first_input.lock().expect("tx_inputs lock");
            (*guard).clone()?
        };

        let outspend = self.api.get_outspend(&prev_txid, prev_vout).await.ok()?;

        if !outspend.spent {
            return None;
        }

        let replacement_txid = outspend.txid?;
        if replacement_txid == original_txid {
            return None;
        }

        // Determine replacement reason by comparing the original and
        // replacement transaction outputs. If the replacement has the
        // same destination outputs, it's a speed-up; otherwise it's
        // a cancellation.
        let reason = match (
            self.api.get_tx(original_txid).await.ok(),
            self.api.get_tx(&replacement_txid).await.ok(),
        ) {
            (Some(original), Some(replacement)) => {
                let original_inputs: std::collections::HashSet<(String, u32)> = original
                    .vin
                    .iter()
                    .map(|v| (v.txid.clone(), v.vout))
                    .collect();
                let replacement_inputs: std::collections::HashSet<(String, u32)> = replacement
                    .vin
                    .iter()
                    .map(|v| (v.txid.clone(), v.vout))
                    .collect();

                if original_inputs == replacement_inputs {
                    ReplacementReason::SpedUp
                } else {
                    ReplacementReason::Cancelled
                }
            }
            _ => ReplacementReason::SpedUp,
        };

        let _ = from_chain; // used by caller for tx_link
        Some(ReplacementTx {
            txid: replacement_txid,
            reason,
        })
    }
}
