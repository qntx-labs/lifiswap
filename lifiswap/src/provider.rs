//! Chain provider trait for multi-chain execution support.
//!
//! Each blockchain family (EVM, SVM, etc.) implements [`Provider`] to handle
//! chain-specific operations such as address validation, balance queries,
//! and transaction execution.

use async_trait::async_trait;

use crate::error::Result;
use crate::types::{
    ChainType, InteractionSettings, LiFiStepExtended, StepExecutorOptions, Token, TokenAmount,
};
use crate::LiFiClient;

/// A chain-specific provider that handles on-chain interactions.
///
/// Implementations live in separate crates (e.g. `lifiswap-evm`, `lifiswap-svm`).
///
/// # Example
///
/// ```ignore
/// use lifiswap::provider::Provider;
///
/// // Register an EVM provider with the client
/// client.set_providers(vec![Box::new(evm_provider)]);
/// ```
#[async_trait]
pub trait Provider: Send + Sync + 'static {
    /// The chain type this provider handles (e.g. `EVM`, `SVM`).
    fn chain_type(&self) -> ChainType;

    /// Validate whether a string is a valid address for this chain type.
    fn is_address(&self, address: &str) -> bool;

    /// Resolve a human-readable name to an address (e.g. ENS, SNS).
    ///
    /// Returns `Ok(None)` if the name cannot be resolved.
    ///
    /// # Errors
    ///
    /// Returns an error if the resolution service is unreachable.
    async fn resolve_address(
        &self,
        name: &str,
        chain_id: Option<u64>,
    ) -> Result<Option<String>>;

    /// Query on-chain token balances for a wallet.
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC call fails.
    async fn get_balance(
        &self,
        wallet_address: &str,
        tokens: &[Token],
    ) -> Result<Vec<TokenAmount>>;

    /// Create a step executor for this chain type.
    ///
    /// The executor handles the full lifecycle of executing a single step:
    /// allowance checks, approvals, signing, broadcasting, and status polling.
    ///
    /// # Errors
    ///
    /// Returns an error if the provider is not properly configured
    /// (e.g. missing signer).
    async fn create_step_executor(
        &self,
        options: StepExecutorOptions,
    ) -> Result<Box<dyn StepExecutor>>;
}

/// Executes a single step within a route.
///
/// Chain-specific crates provide concrete implementations (e.g.
/// `EvmStepExecutor` in `lifiswap-evm`). The execution engine calls
/// [`StepExecutor::execute_step`] for each step in a route.
#[async_trait]
pub trait StepExecutor: Send + Sync {
    /// Execute a single step, mutating its execution state in place.
    ///
    /// # Errors
    ///
    /// Returns an error if any phase of execution fails (balance check,
    /// approval, signing, broadcasting, or status polling).
    async fn execute_step(
        &mut self,
        client: &LiFiClient,
        step: &mut LiFiStepExtended,
    ) -> Result<()>;

    /// Update interaction settings (e.g. disable user prompts for background execution).
    fn set_interaction(&mut self, settings: InteractionSettings);

    /// Whether this executor is allowed to submit transactions.
    fn allow_execution(&self) -> bool;
}
