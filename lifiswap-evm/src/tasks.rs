//! EVM-specific execution tasks.

use alloy::network::{EthereumWallet, TransactionBuilder};
use alloy::primitives::{Address, U256};
use alloy::providers::{Provider as _, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::sol;
use async_trait::async_trait;
use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::execution::status::ActionUpdateParams;
use lifiswap::execution::task::{ExecutionContext, ExecutionTask};
use lifiswap::types::{ExecutionActionStatus, ExecutionActionType, TaskStatus};

sol! {
    #[sol(rpc)]
    contract IERC20 {
        function allowance(address owner, address spender) external view returns (uint256);
        function approve(address spender, uint256 amount) external returns (bool);
    }
}

/// Check ERC-20 token allowance for the approval address.
///
/// Skips if the token is a native token (ETH) or if no approval address is set.
#[derive(Debug, Clone)]
pub struct EvmCheckAllowanceTask {
    rpc_url: String,
}

impl EvmCheckAllowanceTask {
    /// Create a new allowance check task.
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self {
            rpc_url: rpc_url.into(),
        }
    }

    fn is_native_token(address: &str) -> bool {
        address == "0x0000000000000000000000000000000000000000"
            || address.to_lowercase() == "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
    }
}

#[async_trait]
impl ExecutionTask for EvmCheckAllowanceTask {
    async fn should_run(&self, ctx: &ExecutionContext<'_>) -> bool {
        let token_addr = &ctx.step.step.action.from_token.address;
        let has_approval = ctx
            .step
            .step
            .estimate
            .as_ref()
            .and_then(|e| e.approval_address.as_ref())
            .is_some();
        !Self::is_native_token(token_addr) && has_approval
    }

    async fn run(&self, ctx: &mut ExecutionContext<'_>) -> Result<TaskStatus> {
        let from_chain_id = ctx.step.step.action.from_chain_id.0;

        ctx.status_manager.initialize_action(
            ctx.step,
            ExecutionActionType::CheckAllowance,
            from_chain_id,
            ExecutionActionStatus::Started,
        )?;

        let owner: Address = ctx
            .step
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
            .step
            .action
            .from_token
            .address
            .parse()
            .map_err(|_| LiFiError::Validation("Invalid token address.".to_owned()))?;

        let from_amount: U256 = ctx
            .step
            .step
            .action
            .from_amount
            .as_deref()
            .unwrap_or("0")
            .parse()
            .unwrap_or(U256::ZERO);

        let rpc_url: url::Url = self
            .rpc_url
            .parse()
            .map_err(|e| LiFiError::Config(format!("Invalid RPC URL: {e}")))?;
        let provider = ProviderBuilder::new().connect_http(rpc_url);

        let contract = IERC20::new(token_addr, &provider);
        let allowance: U256 = contract
            .allowance(owner, spender)
            .call()
            .await
            .map_err(|e| LiFiError::Provider {
                code: LiFiErrorCode::RpcError,
                message: format!("Failed to check allowance: {e}"),
            })?;

        let sufficient = allowance >= from_amount;

        ctx.status_manager.update_action(
            ctx.step,
            ExecutionActionType::CheckAllowance,
            ExecutionActionStatus::Done,
            None,
        )?;

        if sufficient {
            tracing::debug!(allowance = %allowance, required = %from_amount, "allowance sufficient");
        } else {
            tracing::debug!(allowance = %allowance, required = %from_amount, "allowance insufficient, approval needed");
        }

        Ok(TaskStatus::Completed)
    }
}

/// Approve ERC-20 token spending for the `LiFi` contract.
///
/// Sends an `approve` transaction with `type(uint256).max` as the amount.
#[derive(Debug, Clone)]
pub struct EvmSetAllowanceTask {
    wallet: EthereumWallet,
    rpc_url: String,
}

impl EvmSetAllowanceTask {
    /// Create a new set allowance task.
    pub fn new(wallet: EthereumWallet, rpc_url: impl Into<String>) -> Self {
        Self {
            wallet,
            rpc_url: rpc_url.into(),
        }
    }
}

#[async_trait]
impl ExecutionTask for EvmSetAllowanceTask {
    async fn should_run(&self, ctx: &ExecutionContext<'_>) -> bool {
        let token_addr = &ctx.step.step.action.from_token.address;
        let has_approval = ctx
            .step
            .step
            .estimate
            .as_ref()
            .and_then(|e| e.approval_address.as_ref())
            .is_some();
        !EvmCheckAllowanceTask::is_native_token(token_addr) && has_approval
    }

    async fn run(&self, ctx: &mut ExecutionContext<'_>) -> Result<TaskStatus> {
        let from_chain_id = ctx.step.step.action.from_chain_id.0;

        ctx.status_manager.initialize_action(
            ctx.step,
            ExecutionActionType::SetAllowance,
            from_chain_id,
            ExecutionActionStatus::ActionRequired,
        )?;

        if !ctx.allow_user_interaction {
            return Ok(TaskStatus::Paused);
        }

        let spender: Address = ctx
            .step
            .step
            .estimate
            .as_ref()
            .and_then(|e| e.approval_address.as_deref())
            .expect("approval_address checked in should_run")
            .parse()
            .map_err(|_| LiFiError::Validation("Invalid approval_address.".to_owned()))?;

        let token_addr: Address = ctx
            .step
            .step
            .action
            .from_token
            .address
            .parse()
            .map_err(|_| LiFiError::Validation("Invalid token address.".to_owned()))?;

        let rpc_url: url::Url = self
            .rpc_url
            .parse()
            .map_err(|e| LiFiError::Config(format!("Invalid RPC URL: {e}")))?;
        let provider = ProviderBuilder::new()
            .wallet(self.wallet.clone())
            .connect_http(rpc_url);

        let contract = IERC20::new(token_addr, &provider);
        let tx_hash = contract
            .approve(spender, U256::MAX)
            .send()
            .await
            .map_err(|e| LiFiError::Transaction {
                code: LiFiErrorCode::TransactionFailed,
                message: format!("Approval transaction failed: {e}"),
            })?
            .watch()
            .await
            .map_err(|e| LiFiError::Transaction {
                code: LiFiErrorCode::TransactionFailed,
                message: format!("Approval confirmation failed: {e}"),
            })?;

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
    }
}

/// Sign and broadcast the main swap/bridge transaction.
#[derive(Debug, Clone)]
pub struct EvmSignAndExecuteTask {
    wallet: EthereumWallet,
    rpc_url: String,
}

impl EvmSignAndExecuteTask {
    /// Create a new sign-and-execute task.
    pub fn new(wallet: EthereumWallet, rpc_url: impl Into<String>) -> Self {
        Self {
            wallet,
            rpc_url: rpc_url.into(),
        }
    }
}

#[async_trait]
impl ExecutionTask for EvmSignAndExecuteTask {
    async fn run(&self, ctx: &mut ExecutionContext<'_>) -> Result<TaskStatus> {
        let tx_request =
            ctx.step
                .step
                .transaction_request
                .as_ref()
                .ok_or_else(|| LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "No transaction request data available.".to_owned(),
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

        let data = tx_request
            .data
            .as_deref()
            .ok_or_else(|| LiFiError::Transaction {
                code: LiFiErrorCode::InternalError,
                message: "Transaction request missing 'data'.".to_owned(),
            })?;

        let call_data: alloy::primitives::Bytes = data
            .parse()
            .map_err(|_| LiFiError::Validation("Invalid transaction data hex.".to_owned()))?;

        let value: U256 = tx_request
            .value
            .as_deref()
            .map_or(U256::ZERO, |v| v.parse().unwrap_or(U256::ZERO));

        let gas_limit: Option<u64> = tx_request.gas_limit.as_deref().and_then(|g| g.parse().ok());

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

        let rpc_url: url::Url = self
            .rpc_url
            .parse()
            .map_err(|e| LiFiError::Config(format!("Invalid RPC URL: {e}")))?;
        let provider = ProviderBuilder::new()
            .wallet(self.wallet.clone())
            .connect_http(rpc_url);

        let action_type = if ctx.is_bridge_execution {
            ExecutionActionType::CrossChain
        } else {
            ExecutionActionType::Swap
        };

        let pending = provider
            .send_transaction(tx)
            .await
            .map_err(|e| LiFiError::Transaction {
                code: LiFiErrorCode::TransactionFailed,
                message: format!("Send transaction failed: {e}"),
            })?;

        let tx_hash = *pending.tx_hash();
        tracing::info!(tx = %tx_hash, "transaction sent");

        ctx.status_manager.update_action(
            ctx.step,
            action_type,
            ExecutionActionStatus::Pending,
            Some(ActionUpdateParams {
                tx_hash: Some(format!("{tx_hash:#x}")),
                signed_at: Some(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)),
                ),
                ..Default::default()
            }),
        )?;

        let receipt = pending
            .get_receipt()
            .await
            .map_err(|e| LiFiError::Transaction {
                code: LiFiErrorCode::TransactionFailed,
                message: format!("Transaction receipt failed: {e}"),
            })?;

        if !receipt.status() {
            return Err(LiFiError::Transaction {
                code: LiFiErrorCode::TransactionFailed,
                message: format!("Transaction reverted: {tx_hash:#x}"),
            });
        }

        tracing::info!(tx = %tx_hash, "transaction confirmed");

        Ok(TaskStatus::Completed)
    }
}
