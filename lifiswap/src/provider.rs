//! Chain provider trait for multi-chain execution support.
//!
//! Each blockchain family (EVM, SVM, etc.) implements [`Provider`] to handle
//! chain-specific operations such as address validation, balance queries,
//! and transaction execution.

use std::future::Future;
use std::pin::Pin;

use crate::LiFiClient;
use crate::error::Result;
use crate::types::{
    Chain, ChainType, ExecutionOptions, InteractionSettings, LiFiStepExtended, StepExecutorOptions,
    Token, TokenAmount,
};

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
    fn resolve_address<'a>(
        &'a self,
        name: &'a str,
        chain_id: Option<u64>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + 'a>>;

    /// Query on-chain token balances for a wallet.
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC call fails.
    fn get_balance<'a>(
        &'a self,
        wallet_address: &'a str,
        tokens: &'a [Token],
    ) -> Pin<Box<dyn Future<Output = Result<Vec<TokenAmount>>> + Send + 'a>>;

    /// Create a step executor for this chain type.
    ///
    /// The executor handles the full lifecycle of executing a single step:
    /// allowance checks, approvals, signing, broadcasting, and status polling.
    ///
    /// # Errors
    ///
    /// Returns an error if the provider is not properly configured
    /// (e.g. missing signer).
    fn create_step_executor<'a>(
        &'a self,
        options: StepExecutorOptions,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn StepExecutor>>> + Send + 'a>>;
}

/// Executes a single step within a route.
///
/// Chain-specific crates provide concrete implementations (e.g.
/// `EvmStepExecutor` in `lifiswap-evm`). The execution engine calls
/// [`StepExecutor::execute_step`] for each step in a route.
pub trait StepExecutor: Send + Sync {
    /// Execute a single step, mutating its execution state in place.
    ///
    /// The `provider` reference gives access to on-chain queries (balance,
    /// address validation, etc.) for the step's chain type.
    ///
    /// # Errors
    ///
    /// Returns an error if any phase of execution fails (balance check,
    /// approval, signing, broadcasting, or status polling).
    fn execute_step<'a>(
        &'a mut self,
        client: &'a LiFiClient,
        step: &'a mut LiFiStepExtended,
        provider: &'a dyn Provider,
        execution_options: &'a ExecutionOptions,
        from_chain: &'a Chain,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

    /// Update interaction settings (e.g. disable user prompts for background execution).
    fn set_interaction(&mut self, settings: InteractionSettings);

    /// Whether this executor is allowed to submit transactions.
    fn allow_execution(&self) -> bool;
}
