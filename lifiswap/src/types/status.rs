//! Status request and response types.

use serde::{Deserialize, Serialize};

use super::{ChainId, Token};

/// Request parameters for checking transfer status.
#[derive(Debug, Clone, Serialize, Deserialize, bon::Builder)]
#[serde(rename_all = "camelCase")]
pub struct StatusRequest {
    /// Transaction hash to look up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub tx_hash: Option<String>,
    /// Task ID (for relay transactions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub task_id: Option<String>,
    /// Bridge used for the transfer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub bridge: Option<String>,
    /// Source chain ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_chain: Option<ChainId>,
    /// Destination chain ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_chain: Option<ChainId>,
}

/// Transaction info within a status response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionInfo {
    /// Transaction hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx_hash: Option<String>,
    /// Transaction link (block explorer URL).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx_link: Option<String>,
    /// Chain ID of the transaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<ChainId>,
    /// Token involved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<Token>,
    /// Amount in base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    /// Amount in USD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_usd: Option<String>,
    /// Address involved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// Gas amount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_amount: Option<String>,
    /// Gas amount in USD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_amount_usd: Option<String>,
    /// Gas price.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_price: Option<String>,
    /// Gas used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_used: Option<String>,
    /// Gas token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_token: Option<Token>,
    /// Timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
}

/// Response from the status endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    /// Transaction ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    /// Sending transaction details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sending: Option<TransactionInfo>,
    /// Receiving transaction details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receiving: Option<TransactionInfo>,
    /// `LiFi` explorer link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifi_explorer_link: Option<String>,
    /// Source chain ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_chain_id: Option<ChainId>,
    /// Destination chain ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_chain_id: Option<ChainId>,
    /// Tool/bridge used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Overall status (e.g. "DONE", "PENDING", "FAILED", "`NOT_FOUND`").
    pub status: String,
    /// Substatus for more detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub substatus: Option<String>,
    /// Substatus message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub substatus_message: Option<String>,
    /// Bridge explorer link (for cross-chain transactions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_explorer_link: Option<String>,
    /// Bridge-specific metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}
