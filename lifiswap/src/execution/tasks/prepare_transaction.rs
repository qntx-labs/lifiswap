//! Prepare transaction task — fetches transaction data from the API.

use async_trait::async_trait;

use crate::error::{LiFiError, LiFiErrorCode, Result};
use crate::execution::task::{ExecutionContext, ExecutionTask};
use crate::types::{ExecutionActionStatus, ExecutionActionType, TaskStatus};

/// Fetches the transaction request data for a step via `getStepTransaction`.
///
/// If the step already has `transaction_request` populated (e.g. from a
/// previous attempt), this task skips the API call.
///
/// After preparation, the action status is set to `ActionRequired`.
/// If user interaction is disabled, the pipeline pauses.
#[derive(Debug, Default, Clone, Copy)]
pub struct PrepareTransactionTask;

#[async_trait]
impl ExecutionTask for PrepareTransactionTask {
    async fn run(&self, ctx: &mut ExecutionContext<'_>) -> Result<TaskStatus> {
        let action_type = if ctx.is_bridge_execution {
            ExecutionActionType::CrossChain
        } else {
            ExecutionActionType::Swap
        };

        let _action = ctx.status_manager.find_action(ctx.step, action_type).ok_or(
            LiFiError::Transaction {
                code: LiFiErrorCode::InternalError,
                message: "Unable to prepare transaction. Action not found.".to_owned(),
            },
        )?;

        if ctx.step.step.transaction_request.is_none() {
            let step_for_api = ctx.step.step.clone();
            let updated_step = ctx.client.get_step_transaction(&step_for_api).await?;
            ctx.step.step.transaction_request = updated_step.transaction_request;
        }

        if ctx.step.step.transaction_request.as_ref().and_then(|r| r.data.as_ref()).is_none() {
            return Err(LiFiError::Transaction {
                code: LiFiErrorCode::InternalError,
                message: "Unable to prepare transaction. Transaction request data is not found."
                    .to_owned(),
            });
        }

        ctx.status_manager
            .update_action(ctx.step, action_type, ExecutionActionStatus::ActionRequired, None);

        if !ctx.allow_user_interaction {
            return Ok(TaskStatus::Paused);
        }

        Ok(TaskStatus::Completed)
    }
}
