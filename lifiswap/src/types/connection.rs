//! Connection types for chain-to-chain bridging/swapping availability.

use serde::{Deserialize, Serialize};

use super::{ChainId, Token};

/// Request parameters for getting connections.
#[derive(Debug, Clone, Default, Serialize, Deserialize, bon::Builder)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionsRequest {
    /// Source chain ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_chain: Option<ChainId>,
    /// Source token address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub from_token: Option<String>,
    /// Destination chain ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_chain: Option<ChainId>,
    /// Destination token address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub to_token: Option<String>,
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

/// A connection between two tokens on two chains.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    /// Source chain ID.
    pub from_chain_id: ChainId,
    /// Destination chain ID.
    pub to_chain_id: ChainId,
    /// Source tokens.
    pub from_tokens: Vec<Token>,
    /// Destination tokens.
    pub to_tokens: Vec<Token>,
}

/// Response from the connections endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionsResponse {
    /// Available connections.
    pub connections: Vec<Connection>,
}
