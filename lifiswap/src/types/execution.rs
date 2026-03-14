//! Execution engine types for route execution tracking.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::{FeeCost, GasCost, Token};

/// Callback invoked whenever a route is updated during execution.
pub type UpdateRouteHook = Arc<dyn Fn(&RouteExtended) + Send + Sync>;

/// Parameters passed to the exchange rate update hook.
#[derive(Debug, Clone)]
pub struct ExchangeRateUpdateParams {
    /// Destination token.
    pub to_token: Token,
    /// Previous estimated output amount.
    pub old_to_amount: String,
    /// New estimated output amount.
    pub new_to_amount: String,
}

/// Hook invoked when the exchange rate changes beyond the slippage threshold.
///
/// Should return `true` to accept the new rate, `false` to reject (cancel).
///
/// Wrapped in [`Arc`] so it can be cloned out of shared state for use across
/// async boundaries.
pub type AcceptExchangeRateUpdateHook = Arc<
    dyn Fn(ExchangeRateUpdateParams) -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync,
>;

/// Overall execution status of a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionStatus {
    /// Waiting for processing.
    Pending,
    /// User action needed (e.g. sign transaction).
    ActionRequired,
    /// Execution failed.
    Failed,
    /// Execution completed successfully.
    Done,
}

/// Status of an individual execution action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionActionStatus {
    /// Action has started.
    Started,
    /// User action required.
    ActionRequired,
    /// Message signing required.
    MessageRequired,
    /// Reset required (e.g. allowance reset).
    ResetRequired,
    /// Waiting for confirmation.
    Pending,
    /// Action failed.
    Failed,
    /// Action completed.
    Done,
    /// Action was cancelled.
    Cancelled,
}

/// Type of execution action within a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionActionType {
    /// EIP-2612 permit signing.
    Permit,
    /// Check token allowance.
    CheckAllowance,
    /// Native permit (e.g. DAI-style).
    NativePermit,
    /// Reset token allowance to zero.
    ResetAllowance,
    /// Set token allowance.
    SetAllowance,
    /// On-chain swap.
    Swap,
    /// Cross-chain bridge transaction.
    CrossChain,
    /// Waiting for destination chain confirmation.
    ReceivingChain,
}

/// Transaction method used for execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionMethodType {
    /// Standard on-chain transaction.
    Standard,
    /// Relayed (gasless) transaction.
    Relayed,
    /// Batched transaction (EIP-5792).
    Batched,
}

/// Error details within an execution action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionError {
    /// Error code (numeric or string).
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Optional HTML-formatted error message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub html_message: Option<String>,
}

/// A single action within a step's execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionAction {
    /// Action type.
    #[serde(rename = "type")]
    pub action_type: ExecutionActionType,
    /// Current status.
    pub status: ExecutionActionStatus,
    /// Human-readable status message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Chain ID where this action occurs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<u64>,
    /// Transaction hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx_hash: Option<String>,
    /// Transaction explorer link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx_link: Option<String>,
    /// Task ID (for relay transactions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    /// Transaction method type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx_type: Option<TransactionMethodType>,
    /// Error details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ExecutionError>,
    /// Substatus code from the API.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub substatus: Option<String>,
    /// Substatus message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub substatus_message: Option<String>,
}

/// Execution state of a step, tracking all actions and progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepExecution {
    /// Unix timestamp (ms) when execution started.
    pub started_at: u64,
    /// Unix timestamp (ms) when the transaction was signed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed_at: Option<u64>,
    /// Current execution status.
    pub status: ExecutionStatus,
    /// Ordered list of actions performed.
    pub actions: Vec<ExecutionAction>,
    /// The last action type that was processed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_action_type: Option<ExecutionActionType>,
    /// Actual source amount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_amount: Option<String>,
    /// Actual destination amount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_amount: Option<String>,
    /// Destination token (may differ from estimate after execution).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_token: Option<Token>,
    /// Fee costs incurred.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee_costs: Option<Vec<FeeCost>>,
    /// Gas costs incurred.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_costs: Option<Vec<GasCost>>,
    /// Internal (`LiFi` explorer) transaction link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub internal_tx_link: Option<String>,
    /// External (bridge explorer) transaction link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_tx_link: Option<String>,
    /// Execution-level error (outside of actions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ExecutionError>,
}

/// A `LiFiStep` extended with mutable execution state.
///
/// Implements [`Deref`]/[`DerefMut`] to [`LiFiStep`](super::LiFiStep),
/// so step fields can be accessed directly (e.g. `step.action` instead of
/// `step.step.action`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiFiStepExtended {
    /// The underlying step data.
    #[serde(flatten)]
    pub step: super::LiFiStep,
    /// Mutable execution state (populated during execution).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<StepExecution>,
}

impl std::ops::Deref for LiFiStepExtended {
    type Target = super::LiFiStep;

    fn deref(&self) -> &Self::Target {
        &self.step
    }
}

impl std::ops::DerefMut for LiFiStepExtended {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.step
    }
}

/// A route extended with execution-aware steps.
///
/// Derefs to [`RouteBase`](super::RouteBase) for direct access to shared
/// metadata fields (e.g. `route.id`, `route.from_token`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteExtended {
    /// Shared route metadata.
    #[serde(flatten)]
    pub base: super::RouteBase,
    /// Steps with execution state.
    pub steps: Vec<LiFiStepExtended>,
}

impl std::ops::Deref for RouteExtended {
    type Target = super::RouteBase;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl std::ops::DerefMut for RouteExtended {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl From<super::Route> for RouteExtended {
    fn from(route: super::Route) -> Self {
        Self {
            base: route.base,
            steps: route
                .steps
                .into_iter()
                .map(|s| LiFiStepExtended {
                    step: s,
                    execution: None,
                })
                .collect(),
        }
    }
}

/// Interaction settings controlling user interaction during execution.
#[derive(Debug, Clone, Copy)]
pub struct InteractionSettings {
    /// Allow user interaction (e.g. wallet popups).
    pub allow_interaction: bool,
    /// Allow status updates to propagate.
    pub allow_updates: bool,
    /// Allow transaction execution.
    pub allow_execution: bool,
}

impl Default for InteractionSettings {
    fn default() -> Self {
        Self {
            allow_interaction: true,
            allow_updates: true,
            allow_execution: true,
        }
    }
}

/// Options for configuring route execution behavior.
#[derive(Default)]
pub struct ExecutionOptions {
    /// Hook called whenever the route is updated during execution.
    pub update_route_hook: Option<UpdateRouteHook>,
    /// Hook called when the exchange rate changes beyond the slippage threshold.
    /// Return `true` to accept the new rate, `false` to cancel.
    pub accept_exchange_rate_update_hook: Option<AcceptExchangeRateUpdateHook>,
    /// Whether to execute in the background (no user interaction).
    pub execute_in_background: bool,
}

impl std::fmt::Debug for ExecutionOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionOptions")
            .field(
                "update_route_hook",
                &self.update_route_hook.as_ref().map(|_| ".."),
            )
            .field(
                "accept_exchange_rate_update_hook",
                &self.accept_exchange_rate_update_hook.as_ref().map(|_| ".."),
            )
            .field("execute_in_background", &self.execute_in_background)
            .finish()
    }
}

/// Options passed when creating a step executor.
#[derive(Debug, Clone)]
pub struct StepExecutorOptions {
    /// Route ID this executor belongs to.
    pub route_id: String,
    /// Whether to execute in the background.
    pub execute_in_background: bool,
}

/// Result status of a task in the execution pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task completed, proceed to next.
    Completed,
    /// Task paused, waiting for user interaction.
    Paused,
}
