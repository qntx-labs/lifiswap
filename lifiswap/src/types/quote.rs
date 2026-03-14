//! Quote request and response types.

use serde::{Deserialize, Serialize};

use super::{ChainId, Order};

/// Quote request with `fromAmount` specified.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRequest {
    /// Source chain ID or key.
    pub from_chain: String,
    /// Source token address.
    pub from_token: String,
    /// Sender wallet address.
    pub from_address: String,
    /// Input amount in base units.
    pub from_amount: String,
    /// Destination chain ID or key.
    pub to_chain: String,
    /// Destination token address.
    pub to_token: String,
    /// Receiver wallet address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_address: Option<String>,
    /// Ordering preference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Order>,
    /// Slippage tolerance (0-1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slippage: Option<f64>,
    /// Integrator identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrator: Option<String>,
    /// Referrer address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub referrer: Option<String>,
    /// Integrator fee (0-1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee: Option<f64>,
    /// Allowed bridge keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_bridges: Option<Vec<String>>,
    /// Denied bridge keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_bridges: Option<Vec<String>>,
    /// Preferred bridge keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefer_bridges: Option<Vec<String>>,
    /// Allowed exchange keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_exchanges: Option<Vec<String>>,
    /// Denied exchange keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_exchanges: Option<Vec<String>>,
    /// Preferred exchange keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefer_exchanges: Option<Vec<String>>,
}

/// Quote request using `toAmount` (reverse quote).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteToAmountRequest {
    /// Source chain ID or key.
    pub from_chain: String,
    /// Source token address.
    pub from_token: String,
    /// Sender wallet address.
    pub from_address: String,
    /// Desired output amount in base units.
    pub to_amount: String,
    /// Destination chain ID or key.
    pub to_chain: String,
    /// Destination token address.
    pub to_token: String,
    /// Receiver wallet address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_address: Option<String>,
    /// Ordering preference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Order>,
    /// Slippage tolerance (0-1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slippage: Option<f64>,
    /// Integrator identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrator: Option<String>,
    /// Referrer address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub referrer: Option<String>,
    /// Integrator fee (0-1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee: Option<f64>,
}

/// Contract call specification for `getContractCallsQuote`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractCall {
    /// Target contract address.
    pub call_to: String,
    /// Call data.
    pub call_data: String,
    /// Native token value to send.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_data_value: Option<String>,
    /// Gas limit for this call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_gas_limit: Option<String>,
}

/// Request parameters for contract calls quote.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractCallsQuoteRequest {
    /// Source chain ID or key.
    pub from_chain: String,
    /// Source token address.
    pub from_token: String,
    /// Sender wallet address.
    pub from_address: String,
    /// Destination chain ID or key.
    pub to_chain: String,
    /// Destination token address.
    pub to_token: String,
    /// Input amount in base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_amount: Option<String>,
    /// Desired output amount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_amount: Option<String>,
    /// Contract calls to execute at destination.
    pub contract_calls: Vec<ContractCall>,
    /// Integrator identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrator: Option<String>,
    /// Slippage tolerance (0-1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slippage: Option<f64>,
    /// Referrer address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub referrer: Option<String>,
    /// Integrator fee (0-1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee: Option<f64>,
    /// Allowed bridge keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_bridges: Option<Vec<String>>,
    /// Denied bridge keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_bridges: Option<Vec<String>>,
    /// Preferred bridge keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefer_bridges: Option<Vec<String>>,
    /// Allowed exchange keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_exchanges: Option<Vec<String>>,
    /// Denied exchange keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_exchanges: Option<Vec<String>>,
    /// Preferred exchange keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefer_exchanges: Option<Vec<String>>,
}

/// Request for gas recommendation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GasRecommendationRequest {
    /// Chain ID to get gas recommendation for.
    pub chain_id: ChainId,
    /// Source chain for cross-chain gas estimation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_chain: Option<ChainId>,
    /// Source token for cross-chain gas estimation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_token: Option<String>,
}
