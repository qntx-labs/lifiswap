//! Wait for transaction status task — polls the status API until completion.
//!
//! During polling, intermediate `PENDING` substatus updates are propagated
//! to the action via [`StatusManager`], matching the `TypeScript` SDK behavior.

use std::future::Future;
use std::pin::Pin;

use crate::error::{LiFiError, LiFiErrorCode, Result};
use crate::execution::messages::get_substatus_message;
use crate::execution::status::ExecutionUpdate;
use crate::execution::task::{ExecutionContext, ExecutionTask};
use crate::types::{
    ExecutionActionStatus, ExecutionActionType, ExecutionStatus, TaskStatus, TransferStatus,
};

/// Polls the LI.FI status API until the transaction reaches a terminal state.
///
/// This task handles both same-chain swaps and cross-chain bridge transactions.
/// For bridges, it tracks the `RECEIVING_CHAIN` action separately.
///
/// Intermediate `PENDING` responses update the action's substatus and
/// substatus message so callers can display progress to the user.
#[derive(Debug, Clone, Copy)]
pub struct WaitForTransactionStatusTask {
    action_type: ExecutionActionType,
}

impl WaitForTransactionStatusTask {
    /// Create a task that waits for swap transaction status.
    #[must_use]
    pub const fn swap() -> Self {
        Self {
            action_type: ExecutionActionType::Swap,
        }
    }

    /// Create a task that waits for cross-chain receiving status.
    #[must_use]
    pub const fn receiving_chain() -> Self {
        Self {
            action_type: ExecutionActionType::ReceivingChain,
        }
    }
}

impl ExecutionTask for WaitForTransactionStatusTask {
    fn run<'a>(
        &'a self,
        ctx: &'a mut ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = Result<TaskStatus>> + Send + 'a>> {
        Box::pin(async move {
            let swap_action_type = if ctx.is_bridge_execution {
                ExecutionActionType::CrossChain
            } else {
                ExecutionActionType::Swap
            };

            let tx_hash = ctx
                .status_manager
                .find_action(ctx.step, swap_action_type)
                .and_then(|a| a.tx_hash.as_ref().or(a.task_id.as_ref()))
                .cloned()
                .ok_or_else(|| LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "Transaction hash is undefined.".to_owned(),
                })?;

            let from_chain_id = ctx.step.action.from_chain_id.0;
            let to_chain_id = ctx.step.action.to_chain_id.0;

            let chain_id = if self.action_type == ExecutionActionType::ReceivingChain {
                to_chain_id
            } else {
                from_chain_id
            };

            ctx.status_manager.initialize_action(
                ctx.step,
                self.action_type,
                chain_id,
                ExecutionActionStatus::Pending,
            )?;

            let status_response = poll_transaction_status(
                ctx.client,
                ctx.status_manager,
                ctx.step,
                self.action_type,
                &tx_hash,
            )
            .await?;

            ctx.status_manager.update_action(
                ctx.step,
                self.action_type,
                ExecutionActionStatus::Done,
                Some(crate::execution::status::ActionUpdateParams {
                    chain_id: Some(to_chain_id),
                    tx_hash: status_response
                        .receiving
                        .as_ref()
                        .and_then(|r| r.tx_hash.clone()),
                    tx_link: status_response
                        .receiving
                        .as_ref()
                        .and_then(|r| r.tx_link.clone()),
                    substatus: status_response.substatus.clone(),
                    substatus_message: status_response.substatus_message.clone(),
                    ..Default::default()
                }),
            )?;

            let gas_costs = status_response.sending.as_ref().and_then(|s| {
                let gas_amount = s.gas_amount.as_ref()?;
                let gas_token = s.gas_token.clone()?;
                Some(vec![crate::types::GasCost {
                    cost_type: "SEND".to_owned(),
                    price: s.gas_price.clone(),
                    estimate: s.gas_used.clone(),
                    limit: s.gas_used.clone(),
                    amount: gas_amount.clone(),
                    amount_usd: s.gas_amount_usd.clone(),
                    token: gas_token,
                }])
            });

            ctx.status_manager.update_execution(
                ctx.step,
                ExecutionUpdate {
                    status: Some(ExecutionStatus::Done),
                    from_amount: status_response
                        .sending
                        .as_ref()
                        .and_then(|s| s.amount.clone()),
                    to_amount: status_response
                        .receiving
                        .as_ref()
                        .and_then(|r| r.amount.clone()),
                    to_token: status_response
                        .receiving
                        .as_ref()
                        .and_then(|r| r.token.clone()),
                    gas_costs,
                    internal_tx_link: status_response.lifi_explorer_link.clone(),
                    external_tx_link: status_response.bridge_explorer_link,
                    ..Default::default()
                },
            );

            Ok(TaskStatus::Completed)
        })
    }
}

async fn poll_transaction_status(
    client: &crate::LiFiClient,
    status_manager: &crate::execution::StatusManager,
    step: &mut crate::types::LiFiStepExtended,
    action_type: ExecutionActionType,
    tx_hash: &str,
) -> Result<crate::types::StatusResponse> {
    use crate::types::StatusRequest;

    let mut attempts = 0u32;
    let max_attempts = 120;

    loop {
        let status = client
            .get_status(&StatusRequest::builder().tx_hash(tx_hash).build())
            .await;

        let status = match status {
            Ok(s) => s,
            Err(e) => {
                tracing::debug!(error = %e, "status poll failed, retrying");
                attempts += 1;
                if attempts >= max_attempts {
                    return Err(e);
                }
                let delay = std::cmp::min(5000 + u64::from(attempts) * 1000, 30_000);
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                continue;
            }
        };

        match status.status {
            TransferStatus::Done => return Ok(status),
            TransferStatus::Failed | TransferStatus::Invalid => {
                return Err(LiFiError::Transaction {
                    code: LiFiErrorCode::TransactionFailed,
                    message: format!(
                        "Transaction failed: {}",
                        status
                            .substatus_message
                            .as_deref()
                            .unwrap_or("unknown error")
                    ),
                });
            }
            TransferStatus::Pending => {
                let substatus_msg = status.substatus_message.clone().or_else(|| {
                    status.substatus.as_deref().and_then(|sub| {
                        get_substatus_message("PENDING", Some(sub)).map(String::from)
                    })
                });

                let _ = status_manager.update_action(
                    step,
                    action_type,
                    ExecutionActionStatus::Pending,
                    Some(crate::execution::status::ActionUpdateParams {
                        substatus: status.substatus.clone(),
                        substatus_message: substatus_msg,
                        tx_link: status.bridge_explorer_link.clone(),
                        ..Default::default()
                    }),
                );
            }
            TransferStatus::NotFound => {}
        }

        attempts += 1;
        if attempts >= max_attempts {
            return Err(LiFiError::Transaction {
                code: LiFiErrorCode::Timeout,
                message: "Timed out waiting for transaction status.".to_owned(),
            });
        }
        let delay = std::cmp::min(5000 + u64::from(attempts) * 1000, 30_000);
        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
    }
}
