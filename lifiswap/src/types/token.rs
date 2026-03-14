//! Token-related types.

use serde::{Deserialize, Serialize};

use super::ChainId;

/// Basic token information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    /// Token contract address.
    pub address: String,
    /// Number of decimal places.
    pub decimals: u8,
    /// Token ticker symbol.
    pub symbol: String,
    /// Chain this token resides on.
    pub chain_id: ChainId,
    /// Coin key identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coin_key: Option<String>,
    /// Human-readable token name.
    pub name: String,
    /// Token logo URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_uri: Option<String>,
    /// Current price in USD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_usd: Option<String>,
}

/// Token with a balance amount.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenAmount {
    /// Token contract address.
    pub address: String,
    /// Number of decimal places.
    pub decimals: u8,
    /// Token ticker symbol.
    pub symbol: String,
    /// Chain this token resides on.
    pub chain_id: ChainId,
    /// Coin key identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coin_key: Option<String>,
    /// Human-readable token name.
    pub name: String,
    /// Token logo URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_uri: Option<String>,
    /// Current price in USD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_usd: Option<String>,
    /// Token amount in base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    /// Block number at which the balance was read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_number: Option<u64>,
}

/// Extended token with additional metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenExtended {
    /// Token contract address.
    pub address: String,
    /// Number of decimal places.
    pub decimals: u8,
    /// Token ticker symbol.
    pub symbol: String,
    /// Chain this token resides on.
    pub chain_id: ChainId,
    /// Coin key identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coin_key: Option<String>,
    /// Human-readable token name.
    pub name: String,
    /// Token logo URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_uri: Option<String>,
    /// Current price in USD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_usd: Option<String>,
    /// Whether this token is verified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified: Option<bool>,
}

/// Wallet token with balance and additional info.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletTokenExtended {
    /// Token contract address.
    pub address: String,
    /// Number of decimal places.
    pub decimals: u8,
    /// Token ticker symbol.
    pub symbol: String,
    /// Chain this token resides on.
    pub chain_id: ChainId,
    /// Coin key identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coin_key: Option<String>,
    /// Human-readable token name.
    pub name: String,
    /// Token logo URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_uri: Option<String>,
    /// Current price in USD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_usd: Option<String>,
    /// Token amount in base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    /// Block number at which the balance was read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_number: Option<u64>,
}

/// Request parameters for fetching tokens.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokensRequest {
    /// Filter by chain IDs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chains: Option<String>,
    /// Filter by chain types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_types: Option<String>,
    /// Whether to include extended token info.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extended: Option<bool>,
}

/// Response from the tokens endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokensResponse {
    /// Map of chain ID to list of tokens.
    pub tokens: std::collections::HashMap<String, Vec<Token>>,
}

/// Extended tokens response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokensExtendedResponse {
    /// Map of chain ID to list of extended tokens.
    pub tokens: std::collections::HashMap<String, Vec<TokenExtended>>,
}
