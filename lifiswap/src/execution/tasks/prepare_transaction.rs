//! Prepare transaction task — fetches transaction data from the API.
//!
//! Supports three update paths (mirroring TS SDK's `getUpdatedStep`):
//!
//! 1. **Relay step** → `getRelayerQuote` refreshes typed data + estimate.
//! 2. **Standard with signatures** → `getStepTransactionWithSignatures`
//!    forwards Permit2/EIP-2612 signed data to the API.
//! 3. **Standard** → `getStepTransaction` fetches fresh transaction data.
//!
//! Each path runs `step_comparison` to validate exchange rate changes.

use std::future::Future;
use std::pin::Pin;

use crate::error::{LiFiError, LiFiErrorCode, Result};
use crate::execution::step_comparison::step_comparison;
use crate::execution::task::{ExecutionContext, ExecutionTask};
use crate::types::{
    ExecutionActionStatus, ExecutionActionType, QuoteRequest, SignedLiFiStep, TaskStatus,
};

/// Returns `true` when the step carries non-empty typed data, indicating a
/// relay (gasless) execution path.
fn is_relay_step(ctx: &ExecutionContext<'_>) -> bool {
    ctx.step
        .step
        .typed_data
        .as_ref()
        .is_some_and(|td| !td.is_empty())
}

/// Fetches the transaction request data for a step.
///
/// If the step already has `transaction_request` populated (e.g. from a
/// previous attempt) **and** is not a relay step, this task skips the API
/// call and comparison.
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

            let is_relay = is_relay_step(ctx);

            let needs_refresh = if is_relay {
                true
            } else {
                ctx.step.transaction_request.is_none()
            };

            if needs_refresh {
                let old_step = ctx.step.step.clone();

                let updated_step = if is_relay {
                    get_relay_updated_step(ctx).await?
                } else {
                    get_standard_updated_step(ctx).await?
                };

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

                if is_relay {
                    ctx.step.step.typed_data = updated_step.typed_data;
                }
            }

            if !is_relay
                && ctx
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

async fn get_relay_updated_step(ctx: &ExecutionContext<'_>) -> Result<crate::types::LiFiStep> {
    let action = &ctx.step.action;

    let quote_req = QuoteRequest {
        from_chain: action.from_chain_id.0.to_string(),
        from_token: action.from_token.address.clone(),
        from_address: action.from_address.clone().unwrap_or_default(),
        from_amount: action.from_amount.clone().unwrap_or_default(),
        to_chain: action.to_chain_id.0.to_string(),
        to_token: action.to_token.address.clone(),
        to_address: action.to_address.clone(),
        order: None,
        slippage: action.slippage,
        integrator: None,
        referrer: None,
        fee: None,
        allow_bridges: ctx.step.step.tool.as_ref().map(|t| vec![t.clone()]),
        deny_bridges: None,
        prefer_bridges: None,
        allow_exchanges: None,
        deny_exchanges: None,
        prefer_exchanges: None,
    };

    ctx.client.get_relayer_quote(&quote_req).await
}

async fn get_standard_updated_step(ctx: &ExecutionContext<'_>) -> Result<crate::types::LiFiStep> {
    let old_step = &ctx.step.step;

    let signed_entries: Vec<_> = ctx
        .signed_typed_data
        .iter()
        .filter(|s| s.signature.is_some())
        .cloned()
        .collect();

    if signed_entries.is_empty() {
        ctx.client.get_step_transaction(old_step).await
    } else {
        ctx.client
            .get_step_transaction_with_signatures(&SignedLiFiStep {
                step: old_step.clone(),
                signed_typed_data: Some(signed_entries),
            })
            .await
    }
}
