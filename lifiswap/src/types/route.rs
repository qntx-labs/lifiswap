//! Route-related types.

use serde::{Deserialize, Serialize};

use super::{ChainId, Insurance, LiFiStep, Order, Token, ToolFilter};

/// Route options for customizing route search.
#[derive(Debug, Clone, Default, Serialize, Deserialize, bon::Builder)]
#[serde(rename_all = "camelCase")]
pub struct RouteOptions {
    /// Ordering preference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Order>,
    /// Slippage tolerance (0-1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slippage: Option<f64>,
    /// Maximum price impact tolerance (0-1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_price_impact: Option<f64>,
    /// Integrator fee (0-1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee: Option<f64>,
    /// Referrer address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub referrer: Option<String>,
    /// Bridge filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridges: Option<ToolFilter>,
    /// Exchange/DEX filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exchanges: Option<ToolFilter>,
    /// Allow switching chains during execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_switch_chain: Option<bool>,
    /// Enable Jito bundle for Solana transactions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jito_bundle: Option<bool>,
    /// SVM sponsor address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub svm_sponsor: Option<String>,
}

/// A complete route from source to destination.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    /// Unique route identifier.
    pub id: String,
    /// Source chain ID.
    pub from_chain_id: ChainId,
    /// Destination chain ID.
    pub to_chain_id: ChainId,
    /// Input amount in base units.
    pub from_amount: String,
    /// Output amount in base units.
    pub to_amount: String,
    /// Input amount in USD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_amount_usd: Option<String>,
    /// Output amount in USD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_amount_usd: Option<String>,
    /// Minimum output amount in base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_amount_min: Option<String>,
    /// Source token.
    pub from_token: Token,
    /// Destination token.
    pub to_token: Token,
    /// Sender address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_address: Option<String>,
    /// Receiver address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_address: Option<String>,
    /// Steps to execute this route.
    pub steps: Vec<LiFiStep>,
    /// Route tags (e.g. "RECOMMENDED", "CHEAPEST").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Insurance information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insurance: Option<Insurance>,
    /// Gas cost in USD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_cost_usd: Option<String>,
}

/// Request parameters for getting routes.
#[derive(Debug, Clone, Serialize, Deserialize, bon::Builder)]
#[serde(rename_all = "camelCase")]
pub struct RoutesRequest {
    /// Source chain ID.
    pub from_chain_id: ChainId,
    /// Destination chain ID.
    pub to_chain_id: ChainId,
    /// Source token address.
    #[builder(into)]
    pub from_token_address: String,
    /// Destination token address.
    #[builder(into)]
    pub to_token_address: String,
    /// Input amount in base units.
    #[builder(into)]
    pub from_amount: String,
    /// Sender address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub from_address: Option<String>,
    /// Receiver address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub to_address: Option<String>,
    /// Route options.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<RouteOptions>,
}

/// Response from the routes endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutesResponse {
    /// Available routes.
    pub routes: Vec<Route>,
    /// Unavailable routes with reasons.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_routes: Option<serde_json::Value>,
}
