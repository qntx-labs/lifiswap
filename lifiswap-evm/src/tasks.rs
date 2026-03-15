//! EVM-specific execution tasks.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use alloy::network::TransactionBuilder;
use alloy::primitives::{Address, Bytes, U256};
use alloy::providers::{Provider as _, ProviderBuilder};
use alloy::rpc::types::{TransactionReceipt, TransactionRequest};
use alloy::sol;
use alloy::sol_types::SolCall as _;
use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::execution::status::ActionUpdateParams;
use lifiswap::execution::task::{ExecutionContext, ExecutionTask};
use lifiswap::types::{ExecutionActionStatus, ExecutionActionType, TaskStatus};

use crate::signer::EvmSigner;

sol! {
    #[sol(rpc)]
    contract IERC20 {
        function allowance(address owner, address spender) external view returns (uint256);
        function approve(address spender, uint256 amount) external returns (bool);
    }
}

fn is_native_token(address: &str) -> bool {
    address == "0x0000000000000000000000000000000000000000"
        || address.to_lowercase() == "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Poll for a transaction receipt with retries.
async fn wait_for_receipt(
    rpc_url: &url::Url,
    tx_hash: alloy::primitives::B256,
) -> Result<TransactionReceipt> {
    let provider = ProviderBuilder::new().connect_http(rpc_url.clone());
    for _ in 0..120 {
        match provider.get_transaction_receipt(tx_hash).await {
            Ok(Some(receipt)) => return Ok(receipt),
            Ok(None) => tokio::time::sleep(std::time::Duration::from_secs(2)).await,
            Err(e) => {
                return Err(LiFiError::Transaction {
                    code: LiFiErrorCode::TransactionFailed,
                    message: format!("Failed to fetch receipt: {e}"),
                });
            }
        }
    }
    Err(LiFiError::Transaction {
        code: LiFiErrorCode::Timeout,
        message: format!("Timed out waiting for receipt: {tx_hash}"),
    })
}

/// Send an ERC-20 `approve` transaction via the signer and wait for confirmation.
async fn send_approve(
    signer: &dyn EvmSigner,
    rpc_url: &url::Url,
    token_addr: Address,
    spender: Address,
    amount: U256,
) -> Result<alloy::primitives::B256> {
    let calldata = IERC20::approveCall { spender, amount }.abi_encode();
    let mut tx = TransactionRequest::default();
    tx.set_to(token_addr);
    tx.set_input(Bytes::from(calldata));

    let tx_hash = signer.send_transaction(tx).await?;

    let receipt = wait_for_receipt(rpc_url, tx_hash).await?;
    if !receipt.status() {
        return Err(LiFiError::Transaction {
            code: LiFiErrorCode::TransactionFailed,
            message: format!("Approve transaction reverted: {tx_hash:#x}"),
        });
    }

    Ok(tx_hash)
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
            let token_addr = &ctx.step.action.from_token.address;
            let has_approval = ctx
                .step
                .estimate
                .as_ref()
                .and_then(|e| e.approval_address.as_ref())
                .is_some();
            !is_native_token(token_addr) && has_approval
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

                let tx_hash = send_approve(
                    &*self.signer,
                    &self.rpc_url,
                    token_addr,
                    spender,
                    U256::ZERO,
                )
                .await?;

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

            let tx_hash =
                send_approve(&*self.signer, &self.rpc_url, token_addr, spender, U256::MAX).await?;

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
pub struct EvmSignAndExecuteTask {
    signer: Arc<dyn EvmSigner>,
    rpc_url: url::Url,
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
    pub fn new(signer: Arc<dyn EvmSigner>, rpc_url: url::Url) -> Self {
        Self { signer, rpc_url }
    }
}

impl ExecutionTask for EvmSignAndExecuteTask {
    fn run<'a>(
        &'a self,
        ctx: &'a mut ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = Result<TaskStatus>> + Send + 'a>> {
        Box::pin(async move {
            let tx_request = ctx.step.step.transaction_request.as_ref().ok_or_else(|| {
                LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "No transaction request data available.".to_owned(),
                }
            })?;

            let to_addr: Address = tx_request
                .to
                .as_deref()
                .ok_or_else(|| LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "Transaction request missing 'to' address.".to_owned(),
                })?
                .parse()
                .map_err(|_| LiFiError::Validation("Invalid 'to' address.".to_owned()))?;

            let call_data: Bytes = tx_request
                .data
                .as_deref()
                .ok_or_else(|| LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "Transaction request missing 'data'.".to_owned(),
                })?
                .parse()
                .map_err(|_| LiFiError::Validation("Invalid transaction data hex.".to_owned()))?;

            let value: U256 = tx_request
                .value
                .as_deref()
                .map_or(U256::ZERO, |v| v.parse().unwrap_or(U256::ZERO));

            let gas_limit: Option<u64> =
                tx_request.gas_limit.as_deref().and_then(|g| g.parse().ok());

            let mut tx = TransactionRequest::default();
            tx.set_to(to_addr);
            tx.set_input(call_data);
            tx.set_value(value);

            if let Some(limit) = gas_limit {
                tx.set_gas_limit(limit);
            }

            if let Some(chain_id) = tx_request.chain_id {
                tx.set_chain_id(chain_id);
            }

            let action_type = if ctx.is_bridge_execution {
                ExecutionActionType::CrossChain
            } else {
                ExecutionActionType::Swap
            };

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

            let receipt = wait_for_receipt(&self.rpc_url, tx_hash).await?;

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
