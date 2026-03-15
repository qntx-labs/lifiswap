use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use alloy::network::TransactionBuilder;
use alloy::primitives::{Address, Bytes, U256};
use alloy::rpc::types::TransactionRequest;
use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::execution::status::ActionUpdateParams;
use lifiswap::execution::task::{ExecutionContext, ExecutionTask};
use lifiswap::types::{
    ExecutionActionStatus, ExecutionActionType, TaskStatus, TransactionRequestType,
};

use super::{GAS_BUFFER, apply_tx_hook, estimate_gas, fetch_max_priority_fee, get_tx_link, now_ms};
use crate::executor::Permit2Config;
use crate::is_native_token;
use crate::permit2;
use crate::signer::EvmSigner;

/// Sign and broadcast the main swap/bridge transaction.
///
/// When Permit2 is configured and applicable, wraps the transaction calldata
/// with a Permit2 or native EIP-2612 permit signature before sending.
pub struct EvmSignAndExecuteTask {
    signer: Arc<dyn EvmSigner>,
    rpc_url: url::Url,
    permit2: Option<Permit2Config>,
}

impl std::fmt::Debug for EvmSignAndExecuteTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvmSignAndExecuteTask")
            .field("address", &self.signer.address())
            .finish_non_exhaustive()
    }
}

impl EvmSignAndExecuteTask {
    /// Create a new sign-and-execute task.
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

impl ExecutionTask for EvmSignAndExecuteTask {
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

            let api_tx =
                ctx.step
                    .transaction_request
                    .clone()
                    .ok_or_else(|| LiFiError::Transaction {
                        code: LiFiErrorCode::InternalError,
                        message: "No transaction request data available.".to_owned(),
                    })?;

            let hook = ctx
                .execution_options
                .update_transaction_request_hook
                .as_ref();
            let api_tx = apply_tx_hook(api_tx, TransactionRequestType::Transaction, hook).await?;

            let to_addr: Address = api_tx
                .to
                .as_deref()
                .ok_or_else(|| LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "Transaction request missing 'to' address.".to_owned(),
                })?
                .parse()
                .map_err(|_| LiFiError::Validation("Invalid 'to' address.".to_owned()))?;

            let call_data: Bytes = api_tx
                .data
                .as_deref()
                .ok_or_else(|| LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "Transaction request missing 'data'.".to_owned(),
                })?
                .parse()
                .map_err(|_| LiFiError::Validation("Invalid transaction data hex.".to_owned()))?;

            let value: U256 = api_tx
                .value
                .as_deref()
                .map_or(U256::ZERO, |v| v.parse().unwrap_or(U256::ZERO));

            let gas_limit: Option<u64> = api_tx.gas_limit.as_deref().and_then(|g| g.parse().ok());

            let from_chain_id = ctx.step.action.from_chain_id.0;
            let from_token = ctx.step.action.from_token.address.clone();
            let is_native = is_native_token(&from_token);
            let from_amount: U256 = ctx
                .step
                .action
                .from_amount
                .as_deref()
                .unwrap_or("0")
                .parse()
                .unwrap_or(U256::ZERO);
            let from_token_addr: Address = from_token.parse().unwrap_or(Address::ZERO);

            let signed_native_permit = ctx.signed_typed_data.iter().find(|s| {
                s.typed_data
                    .as_ref()
                    .is_some_and(|td| td.primary_type.as_deref() == Some("Permit"))
                    && s.typed_data.as_ref().is_some_and(|td| {
                        td.domain
                            .as_ref()
                            .and_then(|d| d.get("chainId"))
                            .and_then(serde_json::Value::as_u64)
                            == Some(from_chain_id)
                    })
            });

            let (final_to, final_data) = if let (Some(permit_cfg), Some(signed)) =
                (self.permit2, signed_native_permit)
            {
                let sig_hex = signed.signature.as_deref().unwrap_or("0x");
                let sig_bytes = alloy::hex::decode(sig_hex).map_err(|e| {
                    LiFiError::Validation(format!("Invalid permit signature hex: {e}"))
                })?;

                let msg = signed
                    .typed_data
                    .as_ref()
                    .and_then(|td| td.message.as_ref());
                let deadline_str = msg
                    .and_then(|m| m.get("deadline"))
                    .and_then(|v| v.as_str().or_else(|| v.as_u64().map(|_| "0")))
                    .unwrap_or("0");
                let deadline: U256 = deadline_str.parse().unwrap_or(U256::ZERO);

                let v = if sig_bytes.len() == 65 {
                    sig_bytes[64]
                } else {
                    0
                };
                let mut r = [0u8; 32];
                let mut s = [0u8; 32];
                if sig_bytes.len() >= 64 {
                    r.copy_from_slice(&sig_bytes[..32]);
                    s.copy_from_slice(&sig_bytes[32..64]);
                }

                let wrapped = permit2::encode_native_permit_calldata(
                    from_token_addr,
                    from_amount,
                    deadline,
                    v,
                    r,
                    s,
                    &call_data,
                );
                tracing::info!("wrapping calldata with native EIP-2612 permit");
                (permit_cfg.permit2_proxy, wrapped)
            } else if let Some(permit_cfg) = self.permit2.filter(|_| {
                !is_native
                    && ctx
                        .step
                        .estimate
                        .as_ref()
                        .and_then(|e| e.approval_address.as_ref())
                        .is_some()
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
                        .unwrap_or(false)
            }) {
                ctx.status_manager.update_action(
                    ctx.step,
                    action_type,
                    ExecutionActionStatus::MessageRequired,
                    None,
                )?;

                if !ctx.allow_user_interaction {
                    return Ok(TaskStatus::Paused);
                }

                let owner = self.signer.address();

                let nonce =
                    permit2::fetch_next_nonce(&self.rpc_url, permit_cfg.permit2_proxy, owner)
                        .await?;

                let permit = permit2::PermitTransferFrom {
                    token: from_token_addr,
                    amount: from_amount,
                    spender: permit_cfg.permit2_proxy,
                    nonce,
                    deadline: permit2::default_deadline(),
                };

                let typed_data =
                    permit2::build_permit2_typed_data(&permit, permit_cfg.permit2, from_chain_id);

                let signature = self.signer.sign_typed_data(&typed_data).await?;
                let sig_bytes = alloy::hex::decode(&signature).map_err(|e| {
                    LiFiError::Validation(format!("Invalid Permit2 signature hex: {e}"))
                })?;

                let wrapped = permit2::encode_permit2_calldata(&call_data, &permit, &sig_bytes);

                ctx.status_manager.update_action(
                    ctx.step,
                    action_type,
                    ExecutionActionStatus::ActionRequired,
                    None,
                )?;

                if !ctx.allow_user_interaction {
                    return Ok(TaskStatus::Paused);
                }

                tracing::info!("wrapping calldata with Permit2 signature");
                (permit_cfg.permit2_proxy, wrapped)
            } else {
                (to_addr, call_data)
            };

            let is_permit2_wrapped = final_to != to_addr;

            let mut tx = TransactionRequest::default()
                .with_to(final_to)
                .with_input(final_data)
                .with_value(value);

            if let Some(chain_id) = api_tx.chain_id {
                tx.set_chain_id(chain_id);
            }

            if self.signer.is_local_account() {
                if let Some(fee) = fetch_max_priority_fee(&self.rpc_url).await {
                    tx.set_max_priority_fee_per_gas(fee);
                }
            }

            if is_permit2_wrapped {
                let estimated = estimate_gas(&self.rpc_url, &tx, self.signer.address()).await;
                let original = gas_limit.unwrap_or(0);
                let base = estimated.unwrap_or(original).max(original);
                tx.set_gas_limit(base.saturating_add(GAS_BUFFER));
                tracing::debug!(original, estimated = ?estimated, final_limit = base + GAS_BUFFER, "permit2 gas buffer applied");
            } else if let Some(limit) = gas_limit {
                tx.set_gas_limit(limit);
            }

            let tx_hash = self.signer.send_transaction(tx).await?;
            tracing::info!(tx = %tx_hash, "transaction sent");

            let tx_hash_str = format!("{tx_hash:#x}");
            let tx_link = get_tx_link(ctx.from_chain, &tx_hash_str);
            ctx.status_manager.update_action(
                ctx.step,
                action_type,
                ExecutionActionStatus::Pending,
                Some(ActionUpdateParams {
                    tx_hash: Some(tx_hash_str),
                    tx_link,
                    signed_at: Some(now_ms()),
                    ..Default::default()
                }),
            )?;

            let receipt = self.signer.confirm_transaction(tx_hash).await?;

            if !receipt.status() {
                return Err(LiFiError::Transaction {
                    code: LiFiErrorCode::TransactionFailed,
                    message: format!("Transaction reverted: {tx_hash:#x}"),
                });
            }

            tracing::info!(tx = %tx_hash, "transaction confirmed");

            Ok(TaskStatus::Completed)
        })
    }
}
