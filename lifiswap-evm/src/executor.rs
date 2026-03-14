//! EVM step executor — orchestrates the EVM task pipeline.

use alloy::network::EthereumWallet;
use async_trait::async_trait;
use lifiswap::LiFiClient;
use lifiswap::error::Result;
use lifiswap::execution::status::StatusManager;
use lifiswap::execution::task::{ExecutionContext, TaskPipeline};
use lifiswap::execution::tasks::{
    CheckBalanceTask, PrepareTransactionTask, WaitForTransactionStatusTask,
};
use lifiswap::provider::StepExecutor;
use lifiswap::types::{InteractionSettings, LiFiStepExtended, StepExecutorOptions};

use crate::tasks::{EvmCheckAllowanceTask, EvmSetAllowanceTask, EvmSignAndExecuteTask};

/// EVM-specific step executor.
///
/// Builds a [`TaskPipeline`] with the following sequence:
///
/// 1. `CheckBalanceTask` — verify wallet has sufficient funds
/// 2. `EvmCheckAllowanceTask` — check ERC-20 token allowance
/// 3. `EvmSetAllowanceTask` — approve token spending if needed
/// 4. `PrepareTransactionTask` — fetch transaction data from API
/// 5. `EvmSignAndExecuteTask` — sign and broadcast the transaction
/// 6. `WaitForTransactionStatusTask` — poll status until terminal
pub struct EvmStepExecutor {
    wallet: EthereumWallet,
    rpc_url: String,
    options: StepExecutorOptions,
    interaction: InteractionSettings,
}

impl std::fmt::Debug for EvmStepExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvmStepExecutor")
            .field("rpc_url", &self.rpc_url)
            .field("options", &self.options)
            .field("interaction", &self.interaction)
            .finish_non_exhaustive()
    }
}

impl EvmStepExecutor {
    /// Create a new EVM step executor.
    pub(crate) fn new(
        wallet: EthereumWallet,
        rpc_url: String,
        options: StepExecutorOptions,
    ) -> Self {
        Self {
            wallet,
            rpc_url,
            options,
            interaction: InteractionSettings::default(),
        }
    }

    fn build_pipeline(&self, is_bridge: bool) -> TaskPipeline {
        let mut tasks: Vec<Box<dyn lifiswap::execution::ExecutionTask>> = vec![
            Box::new(CheckBalanceTask),
            Box::new(EvmCheckAllowanceTask::new(self.rpc_url.clone())),
            Box::new(EvmSetAllowanceTask::new(
                self.wallet.clone(),
                self.rpc_url.clone(),
            )),
            Box::new(PrepareTransactionTask),
            Box::new(EvmSignAndExecuteTask::new(
                self.wallet.clone(),
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

#[async_trait]
impl StepExecutor for EvmStepExecutor {
    async fn execute_step(
        &mut self,
        client: &LiFiClient,
        step: &mut LiFiStepExtended,
    ) -> Result<()> {
        let is_bridge = step.step.action.from_chain_id != step.step.action.to_chain_id;

        let status_manager = StatusManager::new(self.options.route_id.clone());
        status_manager.initialize_execution(step);

        let default_opts = lifiswap::types::ExecutionOptions::default();
        let pipeline = self.build_pipeline(is_bridge);

        let mut ctx = ExecutionContext {
            client,
            step,
            status_manager: &status_manager,
            execution_options: &default_opts,
            is_bridge_execution: is_bridge,
            allow_user_interaction: self.interaction.allow_interaction,
        };

        pipeline.run(&mut ctx).await?;

        Ok(())
    }

    fn set_interaction(&mut self, settings: InteractionSettings) {
        self.interaction = settings;
    }

    fn allow_execution(&self) -> bool {
        self.interaction.allow_execution
    }
}
