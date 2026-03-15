use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use alloy::primitives::{Address, U256};
use alloy::providers::ProviderBuilder;
use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::execution::status::ActionUpdateParams;
use lifiswap::execution::task::{ExecutionContext, ExecutionTask};
use lifiswap::types::{ExecutionActionStatus, ExecutionActionType, TaskStatus};

use super::{IERC20, get_tx_link, send_approve};
use crate::executor::Permit2Config;
use crate::is_native_token;
use crate::signer::EvmSigner;

/// Check, reset, and set ERC-20 token allowance for the approval address.
///
/// Flow:
/// 1. Check current allowance on-chain
/// 2. If sufficient, skip
/// 3. If `approval_reset` and existing non-zero allowance, reset to 0 first (for USDT etc.)
/// 4. Approve `U256::MAX`
///
/// Skips entirely if the token is native (ETH) or no approval address is set.
pub struct EvmAllowanceTask {
    signer: Arc<dyn EvmSigner>,
    rpc_url: url::Url,
    permit2: Option<Permit2Config>,
}

impl std::fmt::Debug for EvmAllowanceTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvmAllowanceTask")
            .field("address", &self.signer.address())
            .finish_non_exhaustive()
    }
}

impl EvmAllowanceTask {
    /// Create a new allowance task.
    pub fn new(
        signer: Arc<dyn EvmSigner>,
        rpc_url: url::Url,
        permit2: Option<Permit2Config>,
    ) -> Self {
        Self {
            signer,
            rpc_url,
            permit2,
        }
    }
}

impl ExecutionTask for EvmAllowanceTask {
    fn should_run<'a>(
        &'a self,
        ctx: &'a ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            if is_native_token(&ctx.step.action.from_token.address) {
                return false;
            }

            let Some(estimate) = ctx.step.estimate.as_ref() else {
                return false;
            };

            if estimate.approval_address.is_none() {
                return false;
            }

            if estimate.skip_approval.unwrap_or(false) {
                return false;
            }

            let has_pending_tx = ctx.step.execution.as_ref().is_some_and(|exec| {
                exec.actions.iter().any(|a| {
                    matches!(
                        a.action_type,
                        ExecutionActionType::Swap | ExecutionActionType::CrossChain
                    ) && (a.tx_hash.is_some() || a.task_id.is_some())
                })
            });

            !has_pending_tx
        })
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = Result<TaskStatus>> + Send + 'a>> {
        Box::pin(async move {
            let from_chain_id = ctx.step.action.from_chain_id.0;

            ctx.status_manager.initialize_action(
                ctx.step,
                ExecutionActionType::CheckAllowance,
                from_chain_id,
                ExecutionActionStatus::Started,
            )?;

            let owner: Address = ctx
                .step
                .action
                .from_address
                .as_deref()
                .ok_or_else(|| LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "Missing from_address for allowance check.".to_owned(),
                })?
                .parse()
                .map_err(|_| LiFiError::Validation("Invalid from_address.".to_owned()))?;

            let is_permit2 = self.permit2.is_some()
                && !is_native_token(&ctx.step.action.from_token.address)
                && !ctx
                    .step
                    .estimate
                    .as_ref()
                    .and_then(|e| e.skip_approval)
                    .unwrap_or(false)
                && !ctx
                    .step
                    .estimate
                    .as_ref()
                    .and_then(|e| e.skip_permit)
                    .unwrap_or(false);

            let spender: Address = if is_permit2 {
                self.permit2
                    .ok_or_else(|| LiFiError::Transaction {
                        code: LiFiErrorCode::InternalError,
                        message: "Permit2 config required but not provided.".to_owned(),
                    })?
                    .permit2
            } else {
                ctx.step
                    .estimate
                    .as_ref()
                    .and_then(|e| e.approval_address.as_deref())
                    .ok_or_else(|| LiFiError::Transaction {
                        code: LiFiErrorCode::InternalError,
                        message: "Missing approval_address.".to_owned(),
                    })?
                    .parse()
                    .map_err(|_| LiFiError::Validation("Invalid approval_address.".to_owned()))?
            };

            let token_addr: Address = ctx
                .step
                .action
                .from_token
                .address
                .parse()
                .map_err(|_| LiFiError::Validation("Invalid token address.".to_owned()))?;

            let from_amount: U256 = ctx
                .step
                .action
                .from_amount
                .as_deref()
                .unwrap_or("0")
                .parse()
                .unwrap_or(U256::ZERO);

            let read_provider = ProviderBuilder::new().connect_http(self.rpc_url.clone());
            let contract = IERC20::new(token_addr, &read_provider);
            let allowance: U256 = contract
                .allowance(owner, spender)
                .call()
                .await
                .map_err(|e| LiFiError::Provider {
                    code: LiFiErrorCode::ProviderUnavailable,
                    message: format!("Failed to check allowance: {e}"),
                })?;

            ctx.status_manager.update_action(
                ctx.step,
                ExecutionActionType::CheckAllowance,
                ExecutionActionStatus::Done,
                None,
            )?;

            if allowance >= from_amount {
                tracing::debug!(allowance = %allowance, required = %from_amount, "allowance sufficient");
                return Ok(TaskStatus::Completed);
            }

            tracing::debug!(allowance = %allowance, required = %from_amount, "allowance insufficient");

            let needs_reset = allowance > U256::ZERO
                && ctx
                    .step
                    .estimate
                    .as_ref()
                    .and_then(|e| e.approval_reset)
                    .unwrap_or(false);

            if needs_reset {
                ctx.status_manager.initialize_action(
                    ctx.step,
                    ExecutionActionType::ResetAllowance,
                    from_chain_id,
                    ExecutionActionStatus::ActionRequired,
                )?;

                if !ctx.allow_user_interaction {
                    return Ok(TaskStatus::Paused);
                }

                let hook = ctx
                    .execution_options
                    .update_transaction_request_hook
                    .as_ref();
                let tx_hash =
                    send_approve(&*self.signer, token_addr, spender, U256::ZERO, hook).await?;

                tracing::info!(tx = %tx_hash, "allowance reset to zero");

                ctx.status_manager.update_action(
                    ctx.step,
                    ExecutionActionType::ResetAllowance,
                    ExecutionActionStatus::Done,
                    Some(ActionUpdateParams {
                        tx_hash: Some(format!("{tx_hash:#x}")),
                        ..Default::default()
                    }),
                )?;
            }

            ctx.status_manager.initialize_action(
                ctx.step,
                ExecutionActionType::SetAllowance,
                from_chain_id,
                ExecutionActionStatus::ActionRequired,
            )?;

            if !ctx.allow_user_interaction {
                return Ok(TaskStatus::Paused);
            }

            let approve_amount = if is_permit2 { U256::MAX } else { from_amount };

            let hook = ctx
                .execution_options
                .update_transaction_request_hook
                .as_ref();
            let tx_hash =
                send_approve(&*self.signer, token_addr, spender, approve_amount, hook).await?;

            tracing::info!(tx = %tx_hash, "allowance approved");

            let tx_hash_str = format!("{tx_hash:#x}");
            let tx_link = get_tx_link(ctx.from_chain, &tx_hash_str);
            ctx.status_manager.update_action(
                ctx.step,
                ExecutionActionType::SetAllowance,
                ExecutionActionStatus::Done,
                Some(ActionUpdateParams {
                    tx_hash: Some(tx_hash_str),
                    tx_link,
                    ..Default::default()
                }),
            )?;

            Ok(TaskStatus::Completed)
        })
    }
}
