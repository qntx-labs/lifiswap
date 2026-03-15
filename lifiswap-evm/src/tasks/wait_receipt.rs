use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::execution::status::ActionUpdateParams;
use lifiswap::execution::task::{ExecutionContext, ExecutionTask};
use lifiswap::types::{ExecutionActionStatus, ExecutionActionType, TaskStatus};

use super::get_tx_link;
use crate::signer::EvmSigner;

/// Wait for an on-chain transaction receipt after it has been broadcast.
///
/// This task mirrors the TS SDK's `EthereumStandardWaitForTransactionTask`:
/// it takes the `tx_hash` from the committed action, waits for the receipt
/// via [`EvmSigner::confirm_transaction`], and updates the action if the
/// receipt's tx hash differs (transaction replacement / speed-up).
pub struct EvmWaitForTransactionTask {
    signer: Arc<dyn EvmSigner>,
}

impl std::fmt::Debug for EvmWaitForTransactionTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvmWaitForTransactionTask")
            .field("address", &self.signer.address())
            .finish_non_exhaustive()
    }
}

impl EvmWaitForTransactionTask {
    pub fn new(signer: Arc<dyn EvmSigner>) -> Self {
        Self { signer }
    }
}

impl ExecutionTask for EvmWaitForTransactionTask {
    fn should_run<'a>(
        &'a self,
        ctx: &'a ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            // Run only when there IS a committed tx_hash that hasn't been
            // marked Done yet — i.e. the tx was broadcast but we haven't
            // confirmed the receipt.
            let Some(exec) = ctx.step.execution.as_ref() else {
                return false;
            };
            exec.actions.iter().any(|a| {
                matches!(
                    a.action_type,
                    ExecutionActionType::Swap | ExecutionActionType::CrossChain
                ) && a.tx_hash.is_some()
                    && a.status != ExecutionActionStatus::Done
            })
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

            let tx_hash_str = ctx
                .step
                .execution
                .as_ref()
                .and_then(|exec| {
                    exec.actions.iter().find_map(|a| {
                        if a.action_type == action_type {
                            a.tx_hash.clone()
                        } else {
                            None
                        }
                    })
                })
                .ok_or_else(|| LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "No tx_hash found for receipt waiting.".to_owned(),
                })?;

            let tx_hash: alloy::primitives::TxHash = tx_hash_str.parse().map_err(|_| {
                LiFiError::Validation(format!("Invalid tx_hash for receipt: {tx_hash_str}"))
            })?;

            tracing::info!(tx = %tx_hash, "waiting for transaction receipt");

            let receipt = self.signer.confirm_transaction(tx_hash).await?;

            if !receipt.status() {
                return Err(LiFiError::Transaction {
                    code: LiFiErrorCode::TransactionFailed,
                    message: format!("Transaction reverted: {tx_hash:#x}"),
                });
            }

            // If the receipt's tx hash differs (e.g. replaced/sped-up tx),
            // update the action with the new hash.
            let receipt_hash = format!("{:#x}", receipt.transaction_hash);
            if receipt_hash != tx_hash_str {
                tracing::info!(
                    old_tx = %tx_hash_str,
                    new_tx = %receipt_hash,
                    "transaction was replaced"
                );
                let tx_link = get_tx_link(ctx.from_chain, &receipt_hash);
                ctx.status_manager.update_action(
                    ctx.step,
                    action_type,
                    ExecutionActionStatus::Pending,
                    Some(ActionUpdateParams {
                        tx_hash: Some(receipt_hash),
                        tx_link,
                        ..Default::default()
                    }),
                )?;
            }

            tracing::info!(tx = %receipt.transaction_hash, "transaction confirmed");

            Ok(TaskStatus::Completed)
        })
    }
}
