//! Gas recommendation types.

use serde::{Deserialize, Serialize};

use super::Token;

/// Response from the gas recommendation endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GasRecommendationResponse {
    /// Recommended gas amount in base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended: Option<GasAmount>,
    /// Slow gas amount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slow: Option<GasAmount>,
    /// Average gas amount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub average: Option<GasAmount>,
    /// Fast gas amount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast: Option<GasAmount>,
    /// Token used for gas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<Token>,
}

/// Gas amount recommendation for a speed tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GasAmount {
    /// Amount in base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    /// Amount in USD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_usd: Option<String>,
    /// Token involved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<Token>,
}
