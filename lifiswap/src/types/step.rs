//! Step and action types for route execution.

use serde::{Deserialize, Serialize};

use super::{ChainId, FeeCost, GasCost, Insurance, Token, TransactionRequest};

/// Action describing what a step does.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    /// Source chain ID.
    pub from_chain_id: ChainId,
    /// Destination chain ID.
    pub to_chain_id: ChainId,
    /// Source token.
    pub from_token: Token,
    /// Destination token.
    pub to_token: Token,
    /// Input amount in base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_amount: Option<String>,
    /// Sender address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_address: Option<String>,
    /// Receiver address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_address: Option<String>,
    /// Slippage tolerance (0-1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slippage: Option<f64>,
    /// Destination call data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination_call_data: Option<serde_json::Value>,
}

/// Estimate for a step execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Estimate {
    /// Tool used for this estimate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Input amount in base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_amount: Option<String>,
    /// Input amount in USD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_amount_usd: Option<String>,
    /// Output amount in base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_amount: Option<String>,
    /// Minimum output amount in base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_amount_min: Option<String>,
    /// Output amount in USD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_amount_usd: Option<String>,
    /// Approval address for token allowance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_address: Option<String>,
    /// Estimated execution duration in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_duration: Option<f64>,
    /// Fee costs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee_costs: Option<Vec<FeeCost>>,
    /// Gas costs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_costs: Option<Vec<GasCost>>,
}

/// An included sub-step within a `LiFi` step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncludedStep {
    /// Unique step identifier.
    pub id: String,
    /// Step type (e.g. "swap", "cross", "lifi").
    #[serde(rename = "type")]
    pub step_type: String,
    /// Tool used for this step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Tool details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_details: Option<ToolDetails>,
    /// Action details.
    pub action: Action,
    /// Estimate details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate: Option<Estimate>,
}

/// Tool details for a step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDetails {
    /// Tool key.
    pub key: String,
    /// Tool display name.
    pub name: String,
    /// Tool logo URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_uri: Option<String>,
}

/// EIP-712 typed data for signing (used in relay/gasless flows).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypedData {
    /// EIP-712 domain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<serde_json::Value>,
    /// EIP-712 types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub types: Option<serde_json::Value>,
    /// EIP-712 primary type name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_type: Option<String>,
    /// EIP-712 message value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<serde_json::Value>,
}

/// A single step in a `LiFi` route.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiFiStep {
    /// Unique step identifier.
    pub id: String,
    /// Step type (e.g. "swap", "cross", "lifi").
    #[serde(rename = "type")]
    pub step_type: String,
    /// Tool used for this step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Tool details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_details: Option<ToolDetails>,
    /// Action details.
    pub action: Action,
    /// Estimate details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate: Option<Estimate>,
    /// Sub-steps included within this step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub included_steps: Option<Vec<IncludedStep>>,
    /// Integrator identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrator: Option<String>,
    /// Transaction request data (populated after `getStepTransaction`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction_request: Option<TransactionRequest>,
    /// Execution details (populated during route execution).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<Execution>,
    /// Typed data for gasless/relay signing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed_data: Option<Vec<TypedData>>,
    /// Insurance details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insurance: Option<Insurance>,
}

/// A signed `LiFi` step (includes signature data).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedLiFiStep {
    /// The original step.
    #[serde(flatten)]
    pub step: LiFiStep,
    /// Signatures for typed data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed_typed_data: Option<Vec<SignedTypedData>>,
}

/// Signed typed data (EIP-712 signature result).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedTypedData {
    /// The typed data that was signed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed_data: Option<TypedData>,
    /// The signature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

/// Execution status of a step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Execution {
    /// Current execution status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Execution process details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<Vec<ExecutionProcess>>,
    /// Source token amount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_amount: Option<String>,
    /// Destination token amount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_amount: Option<String>,
    /// Destination token amount in USD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_amount_usd: Option<String>,
    /// Gas used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_used: Option<String>,
    /// Gas price.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_price: Option<String>,
    /// Gas amount in USD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_amount_usd: Option<String>,
}

/// A process within step execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionProcess {
    /// Process type (e.g. `TOKEN_ALLOWANCE`, `SWAP`, `CROSS_CHAIN`).
    #[serde(rename = "type")]
    pub process_type: String,
    /// Process status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Status message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Transaction hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx_hash: Option<String>,
    /// Transaction link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx_link: Option<String>,
    /// Task ID (for relay transactions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    /// Error details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<serde_json::Value>,
    /// Substatus.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub substatus: Option<String>,
    /// Substatus message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub substatus_message: Option<String>,
}
