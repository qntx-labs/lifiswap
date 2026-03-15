//! EVM step executor — orchestrates the EVM task pipeline.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use lifiswap::LiFiClient;
use lifiswap::error::Result;
use lifiswap::execution::status::StatusManager;
use lifiswap::execution::task::{ExecutionContext, TaskPipeline};
use lifiswap::execution::tasks::{
    CheckBalanceTask, PrepareTransactionTask, WaitForTransactionStatusTask,
};
use lifiswap::provider::{Provider, StepExecutor};
use lifiswap::types::{
    ExecutionOptions, InteractionSettings, LiFiStepExtended, StepExecutorOptions,
};

use crate::signer::EvmSigner;
use crate::tasks::{EvmAllowanceTask, EvmSignAndExecuteTask};

/// EVM-specific step executor.
///
/// Builds a [`TaskPipeline`] with the following sequence:
///
/// 1. `CheckBalanceTask` — verify wallet has sufficient funds
/// 2. `EvmAllowanceTask` — check/reset/set allowance as needed
/// 3. `PrepareTransactionTask` — fetch transaction data from API
/// 4. `EvmSignAndExecuteTask` — sign and broadcast the transaction
/// 5. `WaitForTransactionStatusTask` — poll status until terminal
pub struct EvmStepExecutor {
    signer: Arc<dyn EvmSigner>,
    rpc_url: url::Url,
    options: StepExecutorOptions,
    interaction: InteractionSettings,
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
    ) -> Self {
        Self {
            signer,
            rpc_url,
            options,
            interaction: InteractionSettings::default(),
        }
    }

    fn build_pipeline(&self, is_bridge: bool) -> TaskPipeline {
        let mut tasks: Vec<Box<dyn lifiswap::execution::ExecutionTask>> = vec![
            Box::new(CheckBalanceTask),
            Box::new(EvmAllowanceTask::new(
                Arc::clone(&self.signer),
                self.rpc_url.clone(),
            )),
            Box::new(PrepareTransactionTask),
            Box::new(EvmSignAndExecuteTask::new(
                Arc::clone(&self.signer),
                self.rpc_url.clone(),
            )),
        ];

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
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let is_bridge = step.action.from_chain_id != step.action.to_chain_id;

            let status_manager = StatusManager::new(
                self.options.route_id.clone(),
                client.execution_state().clone(),
            );
            status_manager.initialize_execution(step);

            let pipeline = self.build_pipeline(is_bridge);

            let mut ctx = ExecutionContext {
                client,
                step,
                status_manager: &status_manager,
                provider,
                route_id: &self.options.route_id,
                execution_options,
                is_bridge_execution: is_bridge,
                allow_user_interaction: self.interaction.allow_interaction,
            };

            pipeline.run(&mut ctx).await?;

            Ok(())
        })
    }

    fn set_interaction(&mut self, settings: InteractionSettings) {
        self.interaction = settings;
    }

    fn allow_execution(&self) -> bool {
        self.interaction.allow_execution
    }
}
