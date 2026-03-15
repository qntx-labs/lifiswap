//! Prepare transaction task — fetches transaction data from the API.
//!
//! Supports four update paths (mirroring TS SDK's `getUpdatedStep`):
//!
//! 1. **Contract call step** → invokes `getContractCalls` hook, optionally
//!    patches calldata via the patcher API, then re-quotes via
//!    `getContractCallsQuote`.
//! 2. **Relay step** → `getRelayerQuote` refreshes typed data + estimate.
//! 3. **Standard with signatures** → `getStepTransactionWithSignatures`
//!    forwards Permit2/EIP-2612 signed data to the API.
//! 4. **Standard** → `getStepTransaction` fetches fresh transaction data.
//!
//! Each path runs `step_comparison` to validate exchange rate changes.

use std::future::Future;
use std::pin::Pin;

use crate::error::{LiFiError, LiFiErrorCode, Result};
use crate::execution::convert::convert_quote_to_route;
use crate::execution::step_comparison::step_comparison;
use crate::execution::task::{ExecutionContext, ExecutionTask};
use crate::types::{
    CallDataPatch, ContractCallParams, ContractCallsQuoteRequest, ExecutionActionStatus,
    ExecutionActionType, PatchCallDataEntry, QuoteRequest, SignedLiFiStep, StepType, TaskStatus,
    ToolDetails,
};

/// Returns `true` when the step carries non-empty typed data, indicating a
/// relay (gasless) execution path.
fn is_relay_step(ctx: &ExecutionContext<'_>) -> bool {
    ctx.step
        .typed_data
        .as_ref()
        .is_some_and(|td| !td.is_empty())
}

/// Returns `true` when any included sub-step has `type = "custom"`,
/// indicating a contract call execution path.
fn is_contract_call_step(ctx: &ExecutionContext<'_>) -> bool {
    ctx.step
        .included_steps
        .as_ref()
        .is_some_and(|steps| steps.iter().any(|s| s.step_type == StepType::Custom))
}

const PATCHER_MAGIC_NUMBER: &str = "314159265359";

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

            let is_contract_call = is_contract_call_step(ctx);

            if needs_refresh {
                let old_step = ctx.step.step.clone();

                let updated_step = if is_contract_call {
                    get_contract_call_updated_step(ctx).await?
                } else if is_relay {
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
                    ctx.step.typed_data = updated_step.typed_data;
                }
            }

            if !is_relay
                && !is_contract_call
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
        allow_bridges: ctx.step.tool.as_ref().map(|t| vec![t.clone()]),
        deny_bridges: None,
        prefer_bridges: None,
        allow_exchanges: None,
        deny_exchanges: None,
        prefer_exchanges: None,
    };

    ctx.client.get_relayer_quote(&quote_req).await
}

async fn get_contract_call_updated_step(
    ctx: &ExecutionContext<'_>,
) -> Result<crate::types::LiFiStep> {
    let get_contract_calls = ctx
        .execution_options
        .get_contract_calls
        .as_ref()
        .ok_or_else(|| LiFiError::Transaction {
            code: LiFiErrorCode::InternalError,
            message: "Contract call step requires getContractCalls hook.".to_owned(),
        })?;

    let action = &ctx.step.action;
    let estimate = ctx
        .step
        .estimate
        .as_ref()
        .ok_or_else(|| LiFiError::Transaction {
            code: LiFiErrorCode::InternalError,
            message: "Contract call step has no estimate.".to_owned(),
        })?;

    let result = get_contract_calls(ContractCallParams {
        from_chain_id: action.from_chain_id,
        to_chain_id: action.to_chain_id,
        from_token_address: action.from_token.address.clone(),
        to_token_address: action.to_token.address.clone(),
        from_address: action.from_address.clone().unwrap_or_default(),
        to_address: action.to_address.clone(),
        from_amount: action.from_amount.clone().unwrap_or_default(),
        to_amount: estimate.to_amount.clone().unwrap_or_default(),
        slippage: action.slippage,
    })
    .await;

    if result.contract_calls.is_empty() {
        return Err(LiFiError::Transaction {
            code: LiFiErrorCode::InternalError,
            message: "Unable to prepare transaction. Contract calls are not found.".to_owned(),
        });
    }

    let mut contract_calls = result.contract_calls;

    if result.patcher {
        let entries: Vec<PatchCallDataEntry> = contract_calls
            .iter()
            .map(|call| PatchCallDataEntry {
                chain_id: action.to_chain_id,
                from_token_address: call.from_token_address.clone(),
                target_contract_address: call.to_contract_address.clone(),
                call_data_to_patch: call.to_contract_call_data.clone(),
                delegate_call: Some(false),
                patches: vec![CallDataPatch {
                    amount_to_replace: PATCHER_MAGIC_NUMBER.to_owned(),
                }],
                value: None,
            })
            .collect();

        let patched = ctx.client.patch_contract_calls(&entries).await?;

        for (call, patch) in contract_calls.iter_mut().zip(patched.iter()) {
            call.to_contract_address = patch.target.clone();
            call.to_contract_call_data = patch.call_data.clone();
        }
    }

    let mut quote = ctx
        .client
        .get_contract_calls_quote(&ContractCallsQuoteRequest {
            from_chain: action.from_chain_id.0.to_string(),
            from_token: action.from_token.address.clone(),
            from_address: action.from_address.clone().unwrap_or_default(),
            to_chain: action.to_chain_id.0.to_string(),
            to_token: action.to_token.address.clone(),
            from_amount: action.from_amount.clone(),
            to_amount: None,
            contract_calls,
            to_fallback_address: action.to_address.clone(),
            slippage: action.slippage,
            integrator: None,
            referrer: None,
            fee: None,
            allow_bridges: None,
            deny_bridges: None,
            prefer_bridges: None,
            allow_exchanges: None,
            deny_exchanges: None,
            prefer_exchanges: None,
        })
        .await?;

    quote.action.to_token = action.to_token.clone();

    if let Some(tool) = &result.contract_tool {
        let tool_details = ToolDetails {
            key: tool.name.clone(),
            name: tool.name.clone(),
            logo_uri: Some(tool.logo_uri.clone()),
        };
        if let Some(ref mut included) = quote.included_steps
            && let Some(custom) = included
                .iter_mut()
                .find(|s| s.step_type == StepType::Custom)
            {
                custom.tool_details = Some(tool_details.clone());
            }
        quote.tool_details = Some(tool_details);
    }

    let route = convert_quote_to_route(&quote, None)?;
    let mut step = route
        .steps
        .into_iter()
        .next()
        .ok_or_else(|| LiFiError::Transaction {
            code: LiFiErrorCode::InternalError,
            message: "Contract call route has no steps.".to_owned(),
        })?;
    step.id.clone_from(&ctx.step.id);
    Ok(step)
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
