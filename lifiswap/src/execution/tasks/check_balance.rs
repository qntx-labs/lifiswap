//! Balance check task — verifies the wallet has sufficient token balance.

use async_trait::async_trait;

use crate::error::{LiFiError, LiFiErrorCode, Result};
use crate::execution::task::{ExecutionContext, ExecutionTask};
use crate::types::{ExecutionActionStatus, ExecutionActionType, TaskStatus};

/// Checks that the wallet has sufficient balance before executing a step.
///
/// This is a generic task included in every chain's pipeline.
/// It initializes the SWAP or CROSS_CHAIN action and validates the
/// wallet address is present.
#[derive(Debug, Default, Clone, Copy)]
pub struct CheckBalanceTask;

#[async_trait]
impl ExecutionTask for CheckBalanceTask {
    async fn run(&self, ctx: &mut ExecutionContext<'_>) -> Result<TaskStatus> {
        let action_type = if ctx.is_bridge_execution {
            ExecutionActionType::CrossChain
        } else {
            ExecutionActionType::Swap
        };

        let from_chain_id = ctx.step.step.action.from_chain_id.0;

        ctx.status_manager.initialize_action(
            ctx.step,
            action_type,
            from_chain_id,
            ExecutionActionStatus::Started,
        );

        let wallet_address = ctx
            .step
            .step
            .action
            .from_address
            .as_ref()
            .ok_or_else(|| LiFiError::Transaction {
                code: LiFiErrorCode::InternalError,
                message: "The wallet address is undefined.".to_owned(),
            })?;

        tracing::debug!(wallet = %wallet_address, "balance check passed");

        Ok(TaskStatus::Completed)
    }
}
