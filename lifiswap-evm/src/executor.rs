//! EVM step executor — orchestrates the EVM task pipeline.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use alloy::primitives::Address;
use lifiswap::LiFiClient;
use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::execution::run::run_step_pipeline;
use lifiswap::execution::task::TaskPipeline;
use lifiswap::execution::tasks::{
    CheckBalanceTask, PrepareTransactionTask, WaitForTransactionStatusTask,
};
use lifiswap::provider::{Provider, StepExecutor};
use lifiswap::types::{
    Chain, ExecutionOptions, InteractionSettings, LiFiStepExtended, StepExecutorOptions,
};

use crate::signer::EvmSigner;
use crate::tasks::{
    EvmAllowanceTask, EvmBatchedSignAndExecuteTask, EvmCheckPermitsTask, EvmNativePermitTask,
    EvmRelaySignAndExecuteTask, EvmSignAndExecuteTask, EvmWaitForTransactionTask,
};

/// Permit2 contract addresses for a chain.
///
/// Set via [`EvmProvider::with_permit2`] to enable Permit2-based gasless
/// approvals. Addresses are typically obtained from the `/chains` API response
/// (`Chain.permit2` and `Chain.permit2_proxy`).
#[derive(Debug, Clone, Copy)]
pub struct Permit2Config {
    /// Uniswap Permit2 contract address (used for EIP-712 domain).
    pub permit2: Address,
    /// LI.FI `Permit2Proxy` contract address (spender / calldata target).
    pub permit2_proxy: Address,
}

/// EVM-specific step executor.
///
/// Builds a [`TaskPipeline`] with the following sequence:
///
/// 1. `EvmCheckPermitsTask` — sign any pre-existing Permit typed data
/// 2. `CheckBalanceTask` — verify wallet has sufficient funds
/// 3. `EvmAllowanceTask` — check/reset/set allowance as needed
/// 4. `PrepareTransactionTask` — fetch transaction data from API
/// 5. `EvmSignAndExecuteTask` — sign and broadcast (wraps calldata for Permit2)
/// 6. `WaitForTransactionStatusTask` — poll status until terminal
pub struct EvmStepExecutor {
    signer: Arc<dyn EvmSigner>,
    rpc_url: url::Url,
    options: StepExecutorOptions,
    interaction: InteractionSettings,
    permit2: Option<Permit2Config>,
    disable_message_signing: bool,
}

impl std::fmt::Debug for EvmStepExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvmStepExecutor")
            .field("address", &self.signer.address())
            .field("rpc_url", &self.rpc_url.as_str())
            .field("options", &self.options)
            .field("interaction", &self.interaction)
            .finish_non_exhaustive()
    }
}

impl EvmStepExecutor {
    /// Create a new EVM step executor.
    pub(crate) fn new(
        signer: Arc<dyn EvmSigner>,
        rpc_url: url::Url,
        options: StepExecutorOptions,
        permit2: Option<Permit2Config>,
        disable_message_signing: bool,
    ) -> Self {
        Self {
            signer,
            rpc_url,
            options,
            interaction: InteractionSettings::default(),
            permit2,
            disable_message_signing,
        }
    }

    const BATCH_EXCLUDED_TOOLS: &[&str] = &["thorswap"];

    fn build_pipeline(&self, step: &LiFiStepExtended) -> TaskPipeline {
        let is_bridge = step.action.from_chain_id != step.action.to_chain_id;
        let is_relay = step.typed_data.as_ref().is_some_and(|td| !td.is_empty());

        let mut tasks: Vec<Box<dyn lifiswap::execution::ExecutionTask>> = Vec::new();

        let disable_signing =
            self.disable_message_signing || step.step_type != lifiswap::types::StepType::Lifi;
        if !disable_signing {
            tasks.push(Box::new(EvmCheckPermitsTask::new(Arc::clone(&self.signer))));
        }

        let atomicity_not_ready = self
            .options
            .retry_params
            .get("atomicityNotReady")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let tool_supports_batching = step
            .tool
            .as_deref()
            .is_none_or(|t| !Self::BATCH_EXCLUDED_TOOLS.contains(&t));
        let is_batched = !is_relay
            && !atomicity_not_ready
            && self.signer.supports_batching()
            && tool_supports_batching;

        if is_relay {
            tasks.push(Box::new(PrepareTransactionTask));
            tasks.push(Box::new(EvmRelaySignAndExecuteTask::new(Arc::clone(
                &self.signer,
            ))));
        } else if is_batched {
            tasks.push(Box::new(CheckBalanceTask));
            tasks.push(Box::new(PrepareTransactionTask));
            tasks.push(Box::new(EvmBatchedSignAndExecuteTask::new(
                Arc::clone(&self.signer),
                self.rpc_url.clone(),
                self.permit2,
            )));
        } else {
            if !disable_signing {
                tasks.push(Box::new(EvmNativePermitTask::new(
                    Arc::clone(&self.signer),
                    self.rpc_url.clone(),
                    self.permit2,
                )));
            }
            tasks.push(Box::new(CheckBalanceTask));
            tasks.push(Box::new(EvmAllowanceTask::new(
                Arc::clone(&self.signer),
                self.rpc_url.clone(),
                self.permit2,
                disable_signing,
            )));
            tasks.push(Box::new(PrepareTransactionTask));
            tasks.push(Box::new(EvmSignAndExecuteTask::new(
                Arc::clone(&self.signer),
                self.rpc_url.clone(),
                self.permit2,
                disable_signing,
            )));
            tasks.push(Box::new(EvmWaitForTransactionTask::new(Arc::clone(
                &self.signer,
            ))));
        }

        if is_bridge {
            tasks.push(Box::new(WaitForTransactionStatusTask::receiving_chain()));
        } else {
            tasks.push(Box::new(WaitForTransactionStatusTask::swap()));
        }

        TaskPipeline::new(tasks)
    }
}

impl StepExecutor for EvmStepExecutor {
    fn execute_step<'a>(
        &'a mut self,
        client: &'a LiFiClient,
        step: &'a mut LiFiStepExtended,
        provider: &'a dyn Provider,
        execution_options: &'a ExecutionOptions,
        from_chain: &'a Chain,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(ref from_addr) = step.action.from_address {
                let expected: Address = from_addr.parse().map_err(|_| {
                    LiFiError::Validation(format!("Invalid fromAddress: {from_addr}"))
                })?;
                if expected != self.signer.address() {
                    return Err(LiFiError::Transaction {
                        code: LiFiErrorCode::WalletChangedDuringExecution,
                        message: "The wallet address that requested the quote does not match \
                                  the wallet address attempting to sign the transaction."
                            .to_owned(),
                    });
                }
            }

            let pipeline = self.build_pipeline(step);

            run_step_pipeline(
                client,
                step,
                provider,
                execution_options,
                from_chain,
                &self.options,
                self.interaction.allow_interaction,
                pipeline,
                crate::errors::parse_evm_error,
            )
            .await
        })
    }

    fn set_interaction(&mut self, settings: InteractionSettings) {
        self.interaction = settings;
    }

    fn allow_execution(&self) -> bool {
        self.interaction.allow_execution
    }
}
