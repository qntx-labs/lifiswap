//! EVM-specific execution tasks.

mod allowance;
mod batched;
mod native_permit;
mod permits;
mod relay;
mod sign_execute;

pub use allowance::EvmAllowanceTask;
use alloy::network::TransactionBuilder;
use alloy::primitives::{Address, Bytes, U256};
use alloy::providers::{Provider as _, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::sol;
use alloy::sol_types::SolCall as _;
pub use batched::EvmBatchedSignAndExecuteTask;
use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::types::{
    TransactionRequestType, TransactionRequestUpdateHook, TransactionRequestUpdateParams,
};
pub use native_permit::EvmNativePermitTask;
pub use permits::EvmCheckPermitsTask;
pub use relay::EvmRelaySignAndExecuteTask;
pub use sign_execute::EvmSignAndExecuteTask;

use crate::signer::EvmSigner;

sol! {
    #[sol(rpc)]
    contract IERC20 {
        function allowance(address owner, address spender) external view returns (uint256);
        function approve(address spender, uint256 amount) external returns (bool);
    }
}

const GAS_BUFFER: u64 = 300_000;

/// Build a block-explorer transaction link from chain metadata.
///
/// Returns `None` if the chain has no configured explorer URLs.
fn get_tx_link(chain: &lifiswap::types::Chain, tx_hash: &str) -> Option<String> {
    let urls = chain.metamask.as_ref()?.block_explorer_urls.as_ref()?;
    let base = urls.first()?;
    let base = base.trim_end_matches('/');
    Some(format!("{base}/tx/{tx_hash}"))
}

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

/// Fetch `maxPriorityFeePerGas` via `eth_maxPriorityFeePerGas` RPC.
///
/// Returns `None` if the RPC call fails (non-fatal).
async fn fetch_max_priority_fee(rpc_url: &url::Url) -> Option<u128> {
    ProviderBuilder::new()
        .connect_http(rpc_url.clone())
        .get_max_priority_fee_per_gas()
        .await
        .ok()
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

/// Extract chain ID from an EIP-712 domain.
///
/// Mirrors the TS SDK's `getDomainChainId`: checks `domain.chainId` first,
/// falls back to parsing `domain.salt` as a numeric chain ID.
fn get_domain_chain_id(domain: &serde_json::Value) -> Option<u64> {
    if let Some(chain_id) = domain.get("chainId") {
        if let Some(n) = chain_id.as_u64() {
            return Some(n);
        }
        if let Some(s) = chain_id.as_str() {
            if let Ok(n) = s.parse::<u64>() {
                return Some(n);
            }
        }
    }
    if let Some(salt) = domain.get("salt").and_then(|v| v.as_str()) {
        return salt
            .parse::<u64>()
            .ok()
            .or_else(|| u64::from_str_radix(salt.strip_prefix("0x")?, 16).ok());
    }
    None
}
