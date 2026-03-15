use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use alloy::primitives::{Address, Bytes, U256};
use alloy::providers::ProviderBuilder;
use alloy::sol_types::SolCall as _;
use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::execution::status::ActionUpdateParams;
use lifiswap::execution::task::{ExecutionContext, ExecutionTask};
use lifiswap::types::{
    ExecutionActionStatus, ExecutionActionType, TaskStatus, TransactionMethodType,
};

use super::{IERC20, get_tx_link, now_ms};
use crate::executor::Permit2Config;
use crate::is_native_token;
use crate::signer::{BatchCall, EvmSigner};

/// Batched EIP-5792 sign and execute task.
///
/// Combines approve (if needed) and main transaction into a single
/// atomic batch via [`EvmSigner::send_calls`], then polls for completion
/// via [`EvmSigner::get_calls_status`].
pub struct EvmBatchedSignAndExecuteTask {
    signer: Arc<dyn EvmSigner>,
    rpc_url: url::Url,
    permit2: Option<Permit2Config>,
}

impl std::fmt::Debug for EvmBatchedSignAndExecuteTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvmBatchedSignAndExecuteTask")
            .field("address", &self.signer.address())
            .finish_non_exhaustive()
    }
}

impl EvmBatchedSignAndExecuteTask {
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

impl ExecutionTask for EvmBatchedSignAndExecuteTask {
    fn should_run<'a>(
        &'a self,
        ctx: &'a ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move { !ctx.has_committed_transaction() })
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

            ctx.status_manager.update_action(
                ctx.step,
                action_type,
                ExecutionActionStatus::ActionRequired,
                None,
            )?;

            if !ctx.allow_user_interaction {
                return Ok(TaskStatus::Paused);
            }

            let mut calls: Vec<BatchCall> = Vec::new();

            // Build approve call if needed
            if !is_native_token(&ctx.step.action.from_token.address)
                && let Some(estimate) = ctx.step.estimate.as_ref()
                && !estimate.skip_approval.unwrap_or(false)
                && estimate.approval_address.is_some()
            {
                let owner: Address = ctx
                    .step
                    .action
                    .from_address
                    .as_deref()
                    .unwrap_or_default()
                    .parse()
                    .map_err(|_| LiFiError::Validation("Invalid from_address.".to_owned()))?;

                let is_permit2 = self.permit2.is_some() && !estimate.skip_permit.unwrap_or(false);

                let spender: Address = if is_permit2 {
                    self.permit2.expect("permit2 checked above").permit2
                } else {
                    estimate
                        .approval_address
                        .as_deref()
                        .expect("checked above")
                        .parse()
                        .map_err(|_| {
                            LiFiError::Validation("Invalid approval_address.".to_owned())
                        })?
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
                let allowance: U256 =
                    contract
                        .allowance(owner, spender)
                        .call()
                        .await
                        .map_err(|e| LiFiError::Provider {
                            code: LiFiErrorCode::ProviderUnavailable,
                            message: format!("Failed to check allowance: {e}"),
                        })?;

                if allowance < from_amount {
                    let needs_reset =
                        estimate.approval_reset.unwrap_or(false) && allowance > U256::ZERO;
                    if needs_reset {
                        let reset_calldata = IERC20::approveCall {
                            spender,
                            amount: U256::ZERO,
                        }
                        .abi_encode();
                        calls.push(BatchCall {
                            to: token_addr,
                            data: Bytes::from(reset_calldata),
                            value: U256::ZERO,
                        });
                        tracing::debug!("batched: added reset approve(0) call");
                    }

                    let approve_amount = if is_permit2 { U256::MAX } else { from_amount };
                    let calldata = IERC20::approveCall {
                        spender,
                        amount: approve_amount,
                    }
                    .abi_encode();
                    calls.push(BatchCall {
                        to: token_addr,
                        data: Bytes::from(calldata),
                        value: U256::ZERO,
                    });
                    tracing::debug!("batched: added approve call");
                }
            }

            // Build main transaction call
            let api_tx =
                ctx.step
                    .transaction_request
                    .as_ref()
                    .ok_or_else(|| LiFiError::Transaction {
                        code: LiFiErrorCode::InternalError,
                        message: "No transaction request for batched execution.".to_owned(),
                    })?;

            let to_addr: Address = api_tx
                .to
                .as_deref()
                .ok_or_else(|| LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "Missing 'to' in transaction request.".to_owned(),
                })?
                .parse()
                .map_err(|_| LiFiError::Validation("Invalid 'to' address.".to_owned()))?;

            let data = api_tx
                .data
                .as_deref()
                .ok_or_else(|| LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "Missing calldata in transaction request.".to_owned(),
                })?;

            let data_bytes =
                alloy::hex::decode(data.strip_prefix("0x").unwrap_or(data)).map_err(|e| {
                    LiFiError::Transaction {
                        code: LiFiErrorCode::InternalError,
                        message: format!("Invalid calldata hex: {e}"),
                    }
                })?;

            let value: U256 = api_tx
                .value
                .as_deref()
                .and_then(|v| v.parse().ok())
                .unwrap_or(U256::ZERO);

            calls.push(BatchCall {
                to: to_addr,
                data: Bytes::from(data_bytes),
                value,
            });

            // Send batch
            let batch_id = self.signer.send_calls(calls).await?;
            tracing::info!(batch_id = %batch_id, "batched transaction sent");

            ctx.status_manager.update_action(
                ctx.step,
                action_type,
                ExecutionActionStatus::Pending,
                Some(ActionUpdateParams {
                    task_id: Some(batch_id.clone()),
                    tx_type: Some(TransactionMethodType::Batched),
                    signed_at: Some(now_ms()),
                    ..Default::default()
                }),
            )?;

            // Poll for completion
            let receipts = self.signer.get_calls_status(&batch_id).await?;

            if let Some(failed) = receipts.iter().find(|r| !r.success) {
                return Err(LiFiError::Transaction {
                    code: LiFiErrorCode::TransactionFailed,
                    message: format!("Batched transaction reverted: {:#x}", failed.tx_hash),
                });
            }

            let last = receipts.last().ok_or_else(|| LiFiError::Transaction {
                code: LiFiErrorCode::TransactionFailed,
                message: "Batched transaction returned no receipts.".to_owned(),
            })?;

            let tx_hash_str = format!("{:#x}", last.tx_hash);
            let tx_link = get_tx_link(ctx.from_chain, &tx_hash_str);
            ctx.status_manager.update_action(
                ctx.step,
                action_type,
                ExecutionActionStatus::Pending,
                Some(ActionUpdateParams {
                    tx_hash: Some(tx_hash_str),
                    tx_link,
                    ..Default::default()
                }),
            )?;

            tracing::info!(
                tx = %last.tx_hash,
                "batched transaction confirmed"
            );

            Ok(TaskStatus::Completed)
        })
    }
}
