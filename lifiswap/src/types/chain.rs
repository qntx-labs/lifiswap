//! Chain-related types.

use serde::{Deserialize, Serialize};

use super::{ChainId, ChainType, Token};

/// Native currency metadata (for wallet integration).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeCurrency {
    /// Currency name.
    pub name: String,
    /// Currency symbol.
    pub symbol: String,
    /// Decimal places.
    pub decimals: u8,
}

/// Wallet metadata for a chain (e.g. `MetaMask` configuration).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainMetadata {
    /// Chain ID in hex format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<String>,
    /// Block explorer URLs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_explorer_urls: Option<Vec<String>>,
    /// Human-readable chain name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_name: Option<String>,
    /// Native currency info.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_currency: Option<NativeCurrency>,
    /// RPC endpoint URLs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rpc_urls: Option<Vec<String>>,
}

/// Basic chain information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chain {
    /// Unique chain key (e.g. "eth", "pol", "bsc").
    pub key: String,
    /// Human-readable chain name.
    pub name: String,
    /// Blockchain ecosystem type.
    pub chain_type: ChainType,
    /// Native coin symbol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coin: Option<String>,
    /// Numeric chain ID.
    pub id: ChainId,
    /// Whether this is a mainnet chain.
    #[serde(default)]
    pub mainnet: bool,
    /// Chain logo URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_uri: Option<String>,
    /// Token list URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenlist_url: Option<String>,
    /// Faucet URLs (testnets).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faucet_urls: Option<Vec<String>>,
    /// Multicall contract address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multicall_address: Option<String>,
    /// Wallet integration metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metamask: Option<ChainMetadata>,
    /// Native token information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_token: Option<Token>,
}

/// Extended chain with additional metadata returned by the API.
pub type ExtendedChain = Chain;

/// Request parameters for fetching chains.
#[derive(Debug, Clone, Default, Serialize, Deserialize, bon::Builder)]
#[serde(rename_all = "camelCase")]
pub struct ChainsRequest {
    /// Filter by chain types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_types: Option<Vec<ChainType>>,
}

/// Response from the chains endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainsResponse {
    /// List of available chains.
    pub chains: Vec<ExtendedChain>,
}
