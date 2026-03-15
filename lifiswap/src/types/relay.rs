//! Relay (gasless transaction) types.

use serde::{Deserialize, Serialize};

use super::ChainId;

/// Request parameters for relaying a signed transaction.
#[derive(Debug, Clone, Serialize, Deserialize, bon::Builder)]
#[serde(rename_all = "camelCase")]
pub struct RelayRequest {
    /// Signed EIP-712 typed data payloads.
    pub typed_data: Vec<serde_json::Value>,
}

/// Relay response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayResponse {
    /// Response status ("success" or "error").
    pub status: String,
    /// Response data.
    pub data: RelayResponseData,
}

/// Relay response data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayResponseData {
    /// Task ID for tracking.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    /// Transaction link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx_link: Option<String>,
    /// Error code (when status is "error").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
    /// Error message (when status is "error").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Request for checking relayed transaction status.
#[derive(Debug, Clone, Serialize, Deserialize, bon::Builder)]
#[serde(rename_all = "camelCase")]
pub struct RelayStatusRequest {
    /// Task ID to look up.
    #[builder(into)]
    pub task_id: String,
}

/// Response for relayed transaction status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayStatusResponse {
    /// Response status.
    pub status: String,
    /// Response data.
    pub data: RelayStatusResponseData,
}

/// Relayed transaction status data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayStatusResponseData {
    /// Task status (e.g. "PENDING", "DONE", "FAILED").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_status: Option<String>,
    /// Transaction hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx_hash: Option<String>,
    /// Chain ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<ChainId>,
    /// Error code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
    /// Error message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Transaction analytics request.
#[derive(Debug, Clone, Serialize, Deserialize, bon::Builder)]
#[serde(rename_all = "camelCase")]
pub struct TransactionAnalyticsRequest {
    /// Wallet address.
    #[builder(into)]
    pub wallet: String,
    /// Source chain ID filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_chain: Option<ChainId>,
    /// Destination chain ID filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_chain: Option<ChainId>,
    /// Status filter (e.g. "DONE", "PENDING").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// Transaction analytics response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionAnalyticsResponse {
    /// List of transfers.
    #[serde(default)]
    pub transfers: Vec<serde_json::Value>,
}
