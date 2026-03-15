//! Quote request and response types.

use serde::{Deserialize, Serialize};

use super::{ChainId, Order};

/// Quote request with `fromAmount` specified.
#[derive(Debug, Clone, Serialize, Deserialize, bon::Builder)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRequest {
    /// Source chain ID or key.
    #[builder(into)]
    pub from_chain: String,
    /// Source token address.
    #[builder(into)]
    pub from_token: String,
    /// Sender wallet address.
    #[builder(into)]
    pub from_address: String,
    /// Input amount in base units.
    #[builder(into)]
    pub from_amount: String,
    /// Destination chain ID or key.
    #[builder(into)]
    pub to_chain: String,
    /// Destination token address.
    #[builder(into)]
    pub to_token: String,
    /// Receiver wallet address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub to_address: Option<String>,
    /// Ordering preference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Order>,
    /// Slippage tolerance (0-1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slippage: Option<f64>,
    /// Integrator identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub integrator: Option<String>,
    /// Referrer address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
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
#[derive(Debug, Clone, Serialize, Deserialize, bon::Builder)]
#[serde(rename_all = "camelCase")]
pub struct QuoteToAmountRequest {
    /// Source chain ID or key.
    #[builder(into)]
    pub from_chain: String,
    /// Source token address.
    #[builder(into)]
    pub from_token: String,
    /// Sender wallet address.
    #[builder(into)]
    pub from_address: String,
    /// Desired output amount in base units.
    #[builder(into)]
    pub to_amount: String,
    /// Destination chain ID or key.
    #[builder(into)]
    pub to_chain: String,
    /// Destination token address.
    #[builder(into)]
    pub to_token: String,
    /// Receiver wallet address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub to_address: Option<String>,
    /// Ordering preference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Order>,
    /// Slippage tolerance (0-1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slippage: Option<f64>,
    /// Integrator identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub integrator: Option<String>,
    /// Referrer address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub referrer: Option<String>,
    /// Integrator fee (0-1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee: Option<f64>,
}

/// Contract call specification for `getContractCallsQuote`.
#[derive(Debug, Clone, Serialize, Deserialize, bon::Builder)]
#[serde(rename_all = "camelCase")]
pub struct ContractCall {
    /// Input amount in base units.
    #[builder(into)]
    pub from_amount: String,
    /// Source token address.
    #[builder(into)]
    pub from_token_address: String,
    /// Target contract address.
    #[builder(into)]
    pub to_contract_address: String,
    /// Call data to execute on the target contract.
    #[builder(into)]
    pub to_contract_call_data: String,
    /// Gas limit for this call.
    #[builder(into)]
    pub to_contract_gas_limit: String,
}

/// Request parameters for contract calls quote.
#[derive(Debug, Clone, Serialize, Deserialize, bon::Builder)]
#[serde(rename_all = "camelCase")]
pub struct ContractCallsQuoteRequest {
    /// Source chain ID or key.
    #[builder(into)]
    pub from_chain: String,
    /// Source token address.
    #[builder(into)]
    pub from_token: String,
    /// Sender wallet address.
    #[builder(into)]
    pub from_address: String,
    /// Destination chain ID or key.
    #[builder(into)]
    pub to_chain: String,
    /// Destination token address.
    #[builder(into)]
    pub to_token: String,
    /// Input amount in base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_amount: Option<String>,
    /// Desired output amount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_amount: Option<String>,
    /// Contract calls to execute at destination.
    pub contract_calls: Vec<ContractCall>,
    /// Fallback address for failed destination calls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub to_fallback_address: Option<String>,
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
#[derive(Debug, Clone, Serialize, Deserialize, bon::Builder)]
#[serde(rename_all = "camelCase")]
pub struct GasRecommendationRequest {
    /// Chain ID to get gas recommendation for (used as URL path segment).
    #[serde(skip_serializing)]
    pub chain_id: ChainId,
    /// Source chain for cross-chain gas estimation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_chain: Option<ChainId>,
    /// Source token for cross-chain gas estimation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_token: Option<String>,
}
