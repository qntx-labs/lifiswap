//! Shared types used across multiple API endpoints.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A chain identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChainId(pub u64);

impl fmt::Display for ChainId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for ChainId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

/// Blockchain ecosystem type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChainType {
    /// Ethereum Virtual Machine compatible chains.
    EVM,
    /// Solana Virtual Machine.
    SVM,
    /// Bitcoin UTXO model.
    UTXO,
    /// Move Virtual Machine (Sui).
    MVM,
}

impl fmt::Display for ChainType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EVM => write!(f, "EVM"),
            Self::SVM => write!(f, "SVM"),
            Self::UTXO => write!(f, "UTXO"),
            Self::MVM => write!(f, "MVM"),
        }
    }
}

/// Route ordering preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Order {
    /// Recommend the route with the best return.
    Recommended,
    /// Fastest execution time.
    Fastest,
    /// Cheapest gas cost.
    Cheapest,
    /// Most secure route.
    Safest,
}

/// Insurance information for a route.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Insurance {
    /// Current insurance state.
    pub state: String,
    /// Fee amount in USD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee_amount_usd: Option<String>,
}

/// Fee cost information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeCost {
    /// Fee name.
    pub name: String,
    /// Fee description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Fee percentage (0-1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percentage: Option<String>,
    /// Token used for the fee.
    pub token: super::Token,
    /// Fee amount in token units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    /// Fee amount in USD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_usd: Option<String>,
    /// Whether this fee is included in the input amount.
    #[serde(default)]
    pub included: bool,
}

/// Gas cost information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GasCost {
    /// Type of gas cost.
    #[serde(rename = "type")]
    pub cost_type: String,
    /// Estimated gas price.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<String>,
    /// Estimated gas amount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate: Option<String>,
    /// Gas limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<String>,
    /// Gas amount in base units.
    pub amount: String,
    /// Gas cost in USD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_usd: Option<String>,
    /// Token used for gas payment.
    pub token: super::Token,
}

/// Transaction parameters returned by the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionRequest {
    /// Target contract address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// Sender address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Call data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    /// Native token value to send.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Gas price.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_price: Option<String>,
    /// Gas limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_limit: Option<String>,
    /// Chain ID for the transaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<u64>,
}

/// Bridge/exchange tool filter options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolFilter {
    /// Allowed tool keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow: Option<Vec<String>>,
    /// Denied tool keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny: Option<Vec<String>>,
    /// Preferred tool keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefer: Option<Vec<String>>,
}
