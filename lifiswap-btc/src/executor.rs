//! BTC step executor — orchestrates the Bitcoin task pipeline.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

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

use crate::api::BlockchainApi;
use crate::signer::BtcSigner;
use crate::tasks::{BtcConfirmTask, BtcSignTask, BtcTxInputs};

/// Executes a single step on the Bitcoin chain.
///
/// Constructs a task pipeline that:
/// 1. Checks the wallet balance
/// 2. Prepares the transaction via the `LiFi` API
/// 3. Signs the PSBT and broadcasts the raw transaction
/// 4. Waits for on-chain confirmation
/// 5. Waits for the `LiFi` status API to report completion
pub struct BtcStepExecutor {
    signer: Arc<dyn BtcSigner>,
    api: BlockchainApi,
    options: StepExecutorOptions,
    interaction: InteractionSettings,
}

impl std::fmt::Debug for BtcStepExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BtcStepExecutor")
            .field("address", &self.signer.address())
            .field("options", &self.options)
            .field("interaction", &self.interaction)
            .finish_non_exhaustive()
    }
}

impl BtcStepExecutor {
    /// Create a new Bitcoin step executor.
    pub(crate) fn new(
        signer: Arc<dyn BtcSigner>,
        api: BlockchainApi,
        options: StepExecutorOptions,
    ) -> Self {
        Self {
            signer,
            api,
            options,
            interaction: InteractionSettings::default(),
        }
    }

    fn build_pipeline(&self, is_bridge: bool) -> TaskPipeline {
        let status_task = if is_bridge {
            WaitForTransactionStatusTask::receiving_chain()
        } else {
            WaitForTransactionStatusTask::swap()
        };

        let tx_inputs = Arc::new(BtcTxInputs::default());

        TaskPipeline::new(vec![
            Box::new(CheckBalanceTask),
            Box::new(PrepareTransactionTask),
            Box::new(BtcSignTask::new(
                Arc::clone(&self.signer),
                self.api.clone(),
                Arc::clone(&tx_inputs),
            )),
            Box::new(BtcConfirmTask::new(self.api.clone(), tx_inputs)),
            Box::new(status_task),
        ])
    }
}

impl StepExecutor for BtcStepExecutor {
    fn execute_step<'a>(
        &'a mut self,
        client: &'a LiFiClient,
        step: &'a mut LiFiStepExtended,
        provider: &'a dyn Provider,
        execution_options: &'a ExecutionOptions,
        from_chain: &'a Chain,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let signer_address = self.signer.address().to_string();
            if let Some(ref from_address) = step.action.from_address
                && !from_address.eq_ignore_ascii_case(&signer_address)
            {
                return Err(LiFiError::Transaction {
                    code: LiFiErrorCode::WalletChangedDuringExecution,
                    message: "The wallet address that requested the quote does not match \
                              the wallet address attempting to sign the transaction."
                        .to_owned(),
                });
            }

            let is_bridge = step.action.from_chain_id != step.action.to_chain_id;
            let pipeline = self.build_pipeline(is_bridge);

            run_step_pipeline(
                client,
                step,
                provider,
                execution_options,
                from_chain,
                &self.options,
                self.interaction.allow_interaction,
                pipeline,
                crate::errors::parse_bitcoin_error,
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
