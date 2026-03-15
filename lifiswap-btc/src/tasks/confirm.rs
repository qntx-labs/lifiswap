//! Confirm task — polls for Bitcoin transaction confirmation.
//!
//! Mirrors the TS SDK's `BitcoinWaitForTransactionTask`: polls the
//! transaction status via the blockchain API until confirmed on-chain.

use std::future::Future;
use std::pin::Pin;

use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::execution::task::{ExecutionContext, ExecutionTask};
use lifiswap::types::{ExecutionActionStatus, ExecutionActionType, TaskStatus};

use crate::api::BlockchainApi;

/// Polls Bitcoin transaction confirmation status.
///
/// After the sign task broadcasts the transaction, this task polls
/// the blockchain API at 10-second intervals until the transaction
/// is confirmed (included in a block).
pub struct BtcConfirmTask {
    api: BlockchainApi,
}

impl std::fmt::Debug for BtcConfirmTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BtcConfirmTask").finish_non_exhaustive()
    }
}

impl BtcConfirmTask {
    pub(crate) fn new(api: BlockchainApi) -> Self {
        Self { api }
    }
}

/// Polling interval: 10 seconds (matching TS SDK's `pollingIntervalMs: 10_000`).
const POLL_INTERVAL: tokio::time::Duration = tokio::time::Duration::from_secs(10);

/// Maximum time to wait for confirmation: 30 minutes.
const CONFIRM_TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_secs(1800);

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
        Box::pin(async move {
            let action_type = if ctx.is_bridge_execution {
                ExecutionActionType::CrossChain
            } else {
                ExecutionActionType::Swap
            };

            let action = ctx
                .status_manager
                .find_action(ctx.step, action_type)
                .ok_or(LiFiError::Transaction {
                    code: LiFiErrorCode::TransactionUnprepared,
                    message: "Unable to confirm transaction. Action not found.".to_owned(),
                })?;

            let tx_hash = action.tx_hash.as_ref().ok_or(LiFiError::Transaction {
                code: LiFiErrorCode::TransactionUnprepared,
                message: "Transaction hash not set.".to_owned(),
            })?;

            tracing::info!(tx_hash, "Waiting for Bitcoin transaction confirmation");

            let deadline = tokio::time::Instant::now() + CONFIRM_TIMEOUT;

            loop {
                if tokio::time::Instant::now() >= deadline {
                    return Err(LiFiError::Transaction {
                        code: LiFiErrorCode::TransactionExpired,
                        message: "Transaction confirmation timed out after 30 minutes.".to_owned(),
                    });
                }

                match self.api.get_tx_status(tx_hash).await {
                    Ok(status) if status.confirmed => {
                        tracing::info!(
                            tx_hash,
                            block_height = ?status.block_height,
                            "Bitcoin transaction confirmed"
                        );

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
                    Ok(_) => {
                        tracing::debug!(tx_hash, "Transaction not yet confirmed, polling...");
                    }
                    Err(e) => {
                        tracing::warn!(tx_hash, error = %e, "Failed to check tx status");
                    }
                }

                tokio::time::sleep(POLL_INTERVAL).await;
            }
        })
    }
}
