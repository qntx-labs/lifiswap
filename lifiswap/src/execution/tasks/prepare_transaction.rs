//! Prepare transaction task — fetches transaction data from the API.
//!
//! When the step has no cached `transaction_request`, calls
//! `getStepTransaction` and runs `step_comparison` to validate exchange
//! rate changes against the slippage threshold (mirroring TS SDK behavior).

use std::future::Future;
use std::pin::Pin;

use crate::error::{LiFiError, LiFiErrorCode, Result};
use crate::execution::step_comparison::step_comparison;
use crate::execution::task::{ExecutionContext, ExecutionTask};
use crate::types::{ExecutionActionStatus, ExecutionActionType, TaskStatus};

/// Fetches the transaction request data for a step via `getStepTransaction`.
///
/// If the step already has `transaction_request` populated (e.g. from a
/// previous attempt), this task skips the API call and comparison.
///
/// Otherwise:
/// 1. Calls `getStepTransaction` to get fresh transaction data.
/// 2. Runs [`step_comparison`] to check exchange rate changes.
/// 3. If the rate changed beyond slippage and the user rejects, errors out.
/// 4. Updates the step with the new estimate and transaction request.
///
/// After preparation, the action status is set to `ActionRequired`.
/// If user interaction is disabled, the pipeline pauses.
#[derive(Debug, Default, Clone, Copy)]
pub struct PrepareTransactionTask;

impl ExecutionTask for PrepareTransactionTask {
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
                    message: "Unable to prepare transaction. Action not found.".to_owned(),
                })?;

            if ctx.step.transaction_request.is_none() {
                let old_step = ctx.step.step.clone();
                let updated_step = ctx.client.get_step_transaction(&old_step).await?;

                let accept_hook = ctx
                    .execution_options
                    .accept_exchange_rate_update_hook
                    .clone();

                let validated = step_comparison(
                    &old_step,
                    updated_step.clone(),
                    ctx.allow_user_interaction,
                    accept_hook,
                )
                .await?;

                ctx.step.estimate = validated.estimate;
                ctx.step.transaction_request = updated_step.transaction_request;
            }

            if ctx
                .step
                .transaction_request
                .as_ref()
                .and_then(|r| r.data.as_ref())
                .is_none()
            {
                return Err(LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message:
                        "Unable to prepare transaction. Transaction request data is not found."
                            .to_owned(),
                });
            }

            ctx.status_manager.update_action(
                ctx.step,
                action_type,
                ExecutionActionStatus::ActionRequired,
                None,
            )?;

            if !ctx.allow_user_interaction {
                return Ok(TaskStatus::Paused);
            }

            Ok(TaskStatus::Completed)
        })
    }
}
