//! EVM-specific execution tasks.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use alloy::network::TransactionBuilder;
use alloy::primitives::{Address, Bytes, U256};
use alloy::providers::{Provider as _, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::sol;
use alloy::sol_types::SolCall as _;
use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::execution::status::ActionUpdateParams;
use lifiswap::execution::task::{ExecutionContext, ExecutionTask};
use lifiswap::types::{
    ExecutionActionStatus, ExecutionActionType, TaskStatus, TransactionRequestType,
    TransactionRequestUpdateHook, TransactionRequestUpdateParams,
};

use crate::executor::Permit2Config;
use crate::permit2;
use crate::signer::EvmSigner;

sol! {
    #[sol(rpc)]
    contract IERC20 {
        function allowance(address owner, address spender) external view returns (uint256);
        function approve(address spender, uint256 amount) external returns (bool);
    }
}

fn is_native_token(address: &str) -> bool {
    address.parse::<Address>().is_ok_and(|a| a.is_zero())
        || address.eq_ignore_ascii_case("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE")
}

const GAS_BUFFER: u64 = 300_000;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Estimate gas for a transaction via `eth_estimateGas`.
///
/// Returns `None` if the estimation fails (non-fatal — the caller falls back
/// to the original gas limit from the API).
async fn estimate_gas(rpc_url: &url::Url, tx: &TransactionRequest, from: Address) -> Option<u64> {
    let provider = ProviderBuilder::new().connect_http(rpc_url.clone());
    let mut est_tx = tx.clone();
    est_tx.set_from(from);
    match provider.estimate_gas(est_tx).await {
        Ok(gas) => Some(gas),
        Err(e) => {
            tracing::warn!(error = %e, "gas estimation failed, using original limit");
            None
        }
    }
}

/// Send an ERC-20 `approve` transaction via the signer and wait for confirmation.
async fn send_approve(
    signer: &dyn EvmSigner,
    token_addr: Address,
    spender: Address,
    amount: U256,
    hook: Option<&TransactionRequestUpdateHook>,
) -> Result<alloy::primitives::B256> {
    let calldata = IERC20::approveCall { spender, amount }.abi_encode();

    let api_tx = lifiswap::types::TransactionRequest {
        to: Some(format!("{token_addr:#x}")),
        from: None,
        data: Some(format!("0x{}", alloy::hex::encode(&calldata))),
        value: None,
        gas_price: None,
        gas_limit: None,
        chain_id: None,
    };

    let api_tx = apply_tx_hook(api_tx, TransactionRequestType::Approve, hook).await?;

    let input: Bytes = api_tx
        .data
        .as_deref()
        .and_then(|d| d.parse().ok())
        .unwrap_or_else(|| Bytes::from(calldata));

    let mut tx = TransactionRequest::default()
        .with_to(token_addr)
        .with_input(input);
    if let Some(limit) = api_tx
        .gas_limit
        .as_deref()
        .and_then(|g| g.parse::<u64>().ok())
    {
        tx.set_gas_limit(limit);
    }

    let tx_hash = signer.send_transaction(tx).await?;

    let receipt = signer.confirm_transaction(tx_hash).await?;
    if !receipt.status() {
        return Err(LiFiError::Transaction {
            code: LiFiErrorCode::TransactionFailed,
            message: format!("Approve transaction reverted: {tx_hash:#x}"),
        });
    }

    Ok(tx_hash)
}

/// Apply the user's transaction request update hook, if present.
async fn apply_tx_hook(
    tx: lifiswap::types::TransactionRequest,
    request_type: TransactionRequestType,
    hook: Option<&TransactionRequestUpdateHook>,
) -> Result<lifiswap::types::TransactionRequest> {
    match hook {
        Some(hook) => Ok(hook(TransactionRequestUpdateParams {
            request_type,
            transaction: tx,
        })
        .await),
        None => Ok(tx),
    }
}

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
    pub fn new(signer: Arc<dyn EvmSigner>, rpc_url: url::Url) -> Self {
        Self { signer, rpc_url }
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

            let spender: Address = ctx
                .step
                .estimate
                .as_ref()
                .and_then(|e| e.approval_address.as_deref())
                .ok_or_else(|| LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "Missing approval_address.".to_owned(),
                })?
                .parse()
                .map_err(|_| LiFiError::Validation("Invalid approval_address.".to_owned()))?;

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

            let hook = ctx
                .execution_options
                .update_transaction_request_hook
                .as_ref();
            let tx_hash = send_approve(&*self.signer, token_addr, spender, U256::MAX, hook).await?;

            tracing::info!(tx = %tx_hash, "allowance approved");

            ctx.status_manager.update_action(
                ctx.step,
                ExecutionActionType::SetAllowance,
                ExecutionActionStatus::Done,
                Some(ActionUpdateParams {
                    tx_hash: Some(format!("{tx_hash:#x}")),
                    ..Default::default()
                }),
            )?;

            Ok(TaskStatus::Completed)
        })
    }
}

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
    fn run<'a>(
        &'a self,
        ctx: &'a mut ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = Result<TaskStatus>> + Send + 'a>> {
        Box::pin(async move {
            let api_tx = ctx.step.step.transaction_request.clone().ok_or_else(|| {
                LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "No transaction request data available.".to_owned(),
                }
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

            let action_type = if ctx.is_bridge_execution {
                ExecutionActionType::CrossChain
            } else {
                ExecutionActionType::Swap
            };

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

            ctx.status_manager.update_action(
                ctx.step,
                action_type,
                ExecutionActionStatus::Pending,
                Some(ActionUpdateParams {
                    tx_hash: Some(format!("{tx_hash:#x}")),
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

/// Sign EIP-712 typed data and relay via the LI.FI relayer (gasless).
///
/// Flow:
/// 1. Extract unsigned `typed_data` from the step
/// 2. Sign each entry via [`EvmSigner::sign_typed_data`]
/// 3. Submit signed data to `client.relay_transaction()`
/// 4. Update action with `task_id` for status polling
pub struct EvmRelaySignAndExecuteTask {
    signer: Arc<dyn EvmSigner>,
}

impl std::fmt::Debug for EvmRelaySignAndExecuteTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvmRelaySignAndExecuteTask")
            .field("address", &self.signer.address())
            .finish_non_exhaustive()
    }
}

impl EvmRelaySignAndExecuteTask {
    pub fn new(signer: Arc<dyn EvmSigner>) -> Self {
        Self { signer }
    }
}

impl ExecutionTask for EvmRelaySignAndExecuteTask {
    fn run<'a>(
        &'a self,
        ctx: &'a mut ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = Result<TaskStatus>> + Send + 'a>> {
        Box::pin(async move {
            let unsigned = ctx
                .step
                .step
                .typed_data
                .as_ref()
                .ok_or_else(|| LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "No typed data found for relay transaction.".to_owned(),
                })?
                .clone();

            if unsigned.is_empty() {
                return Err(LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "Typed data array is empty.".to_owned(),
                });
            }

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

            let mut signed_data: Vec<serde_json::Value> = Vec::with_capacity(unsigned.len());

            for td in &unsigned {
                let signature = self.signer.sign_typed_data(td).await?;

                let mut entry = serde_json::to_value(td).map_err(|e| LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: format!("Failed to serialize typed data: {e}"),
                })?;

                if let serde_json::Value::Object(ref mut map) = entry {
                    map.insert("signature".to_owned(), serde_json::Value::String(signature));
                }

                signed_data.push(entry);
            }

            ctx.status_manager.update_action(
                ctx.step,
                action_type,
                ExecutionActionStatus::Pending,
                None,
            )?;

            let relay_resp = ctx
                .client
                .relay_transaction(&lifiswap::types::RelayRequest {
                    typed_data: signed_data,
                })
                .await?;

            ctx.status_manager.update_action(
                ctx.step,
                action_type,
                ExecutionActionStatus::Pending,
                Some(ActionUpdateParams {
                    tx_hash: relay_resp.task_id.clone(),
                    signed_at: Some(now_ms()),
                    tx_link: relay_resp.tx_link.clone(),
                    ..Default::default()
                }),
            )?;

            tracing::info!(
                task_id = ?relay_resp.task_id,
                "relay transaction submitted"
            );

            Ok(TaskStatus::Completed)
        })
    }
}

/// Sign any `Permit` typed data entries from the step before execution.
///
/// Filters `step.typedData` for entries with `primaryType == "Permit"`,
/// signs each one via [`EvmSigner::sign_typed_data`], and stores the
/// results in [`ExecutionContext::signed_typed_data`] for downstream tasks.
pub struct EvmCheckPermitsTask {
    signer: Arc<dyn EvmSigner>,
}

impl std::fmt::Debug for EvmCheckPermitsTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvmCheckPermitsTask")
            .field("address", &self.signer.address())
            .finish_non_exhaustive()
    }
}

impl EvmCheckPermitsTask {
    pub fn new(signer: Arc<dyn EvmSigner>) -> Self {
        Self { signer }
    }
}

impl ExecutionTask for EvmCheckPermitsTask {
    fn should_run<'a>(
        &'a self,
        ctx: &'a ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            ctx.step.step.typed_data.as_ref().is_some_and(|tds| {
                tds.iter()
                    .any(|td| td.primary_type.as_deref() == Some("Permit"))
            })
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
                ExecutionActionType::Permit,
                from_chain_id,
                ExecutionActionStatus::Started,
            )?;

            let permit_entries: Vec<_> = ctx
                .step
                .step
                .typed_data
                .as_ref()
                .map(|tds| {
                    tds.iter()
                        .filter(|td| td.primary_type.as_deref() == Some("Permit"))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();

            for td in &permit_entries {
                ctx.status_manager.update_action(
                    ctx.step,
                    ExecutionActionType::Permit,
                    ExecutionActionStatus::ActionRequired,
                    None,
                )?;

                if !ctx.allow_user_interaction {
                    return Ok(TaskStatus::Paused);
                }

                let signature = self.signer.sign_typed_data(td).await?;

                ctx.signed_typed_data
                    .push(lifiswap::types::SignedTypedData {
                        typed_data: Some(td.clone()),
                        signature: Some(signature),
                    });
            }

            ctx.status_manager.update_action(
                ctx.step,
                ExecutionActionType::Permit,
                ExecutionActionStatus::Done,
                None,
            )?;

            Ok(TaskStatus::Completed)
        })
    }
}
