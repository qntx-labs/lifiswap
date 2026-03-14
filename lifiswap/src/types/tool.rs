//! Tool (bridge/exchange) types.

use serde::{Deserialize, Serialize};

/// Request parameters for fetching available tools.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsRequest {
    /// Filter by chain IDs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chains: Option<String>,
}

/// A bridge tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bridge {
    /// Unique bridge key.
    pub key: String,
    /// Human-readable bridge name.
    pub name: String,
    /// Bridge logo URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_uri: Option<String>,
    /// Supported chain IDs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported_chains: Option<Vec<SupportedChain>>,
}

/// An exchange (DEX) tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Exchange {
    /// Unique exchange key.
    pub key: String,
    /// Human-readable exchange name.
    pub name: String,
    /// Exchange logo URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_uri: Option<String>,
    /// Supported chain IDs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported_chains: Option<Vec<SupportedChain>>,
}

/// A supported chain entry within a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportedChain {
    /// Chain ID.
    pub id: super::ChainId,
    /// Chain key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

/// Response from the tools endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsResponse {
    /// Available bridges.
    pub bridges: Vec<Bridge>,
    /// Available exchanges.
    pub exchanges: Vec<Exchange>,
}
