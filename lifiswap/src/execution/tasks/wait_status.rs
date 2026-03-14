//! Wait for transaction status task — polls the status API until completion.

use async_trait::async_trait;

use crate::error::{LiFiError, LiFiErrorCode, Result};
use crate::execution::status::ExecutionUpdate;
use crate::execution::task::{ExecutionContext, ExecutionTask};
use crate::types::{ExecutionActionStatus, ExecutionActionType, ExecutionStatus, TaskStatus};

/// Polls the LI.FI status API until the transaction reaches a terminal state.
///
/// This task handles both same-chain swaps and cross-chain bridge transactions.
/// For bridges, it tracks the `RECEIVING_CHAIN` action separately.
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

#[async_trait]
impl ExecutionTask for WaitForTransactionStatusTask {
    async fn run(&self, ctx: &mut ExecutionContext<'_>) -> Result<TaskStatus> {
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

        let from_chain_id = ctx.step.step.action.from_chain_id.0;
        let to_chain_id = ctx.step.step.action.to_chain_id.0;

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

        let status_response = poll_transaction_status(ctx.client, &tx_hash).await?;

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
                ..Default::default()
            },
        );

        Ok(TaskStatus::Completed)
    }
}

async fn poll_transaction_status(
    client: &crate::LiFiClient,
    tx_hash: &str,
) -> Result<crate::types::StatusResponse> {
    use crate::types::StatusRequest;

    let mut attempts = 0u32;
    let max_attempts = 120;

    loop {
        let status = client
            .get_status(&StatusRequest::builder().tx_hash(tx_hash).build())
            .await?;

        match status.status.as_str() {
            "DONE" => return Ok(status),
            "FAILED" | "INVALID" => {
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
            _ => {
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
    }
}
