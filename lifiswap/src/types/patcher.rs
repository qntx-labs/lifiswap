//! Patcher (contract call patching) types.

use serde::{Deserialize, Serialize};

use super::ChainId;

/// A single patch entry describing which amount to replace.
#[derive(Debug, Clone, Serialize, Deserialize, bon::Builder)]
#[serde(rename_all = "camelCase")]
pub struct CallDataPatch {
    /// The amount string to locate and replace in the call data.
    #[builder(into)]
    pub amount_to_replace: String,
}

/// A single contract call to patch.
#[derive(Debug, Clone, Serialize, Deserialize, bon::Builder)]
#[serde(rename_all = "camelCase")]
pub struct PatchCallDataEntry {
    /// Chain ID where the contract call will be executed.
    pub chain_id: ChainId,
    /// Source token address.
    #[builder(into)]
    pub from_token_address: String,
    /// Target contract address.
    #[builder(into)]
    pub target_contract_address: String,
    /// The raw call data to patch.
    #[builder(into)]
    pub call_data_to_patch: String,
    /// List of patches to apply.
    pub patches: Vec<CallDataPatch>,
    /// Native token value to send with the call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub value: Option<String>,
    /// Whether this is a delegate call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delegate_call: Option<bool>,
}

/// Response for a single patched contract call.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchContractCallsResponse {
    /// Target contract address.
    pub target: String,
    /// Native token value (as string, since the API returns bigint).
    pub value: String,
    /// Patched call data.
    pub call_data: String,
    /// Whether this call is allowed to fail.
    pub allow_failure: bool,
    /// Whether this is a delegate call.
    pub is_delegate_call: bool,
}
