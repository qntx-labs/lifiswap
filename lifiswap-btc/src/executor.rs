//! BTC step executor — orchestrates the Bitcoin task pipeline.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use lifiswap::LiFiClient;
use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::execution::status::{ExecutionUpdate, StatusManager};
use lifiswap::execution::task::{ExecutionContext, TaskPipeline};
use lifiswap::execution::tasks::{
    CheckBalanceTask, PrepareTransactionTask, WaitForTransactionStatusTask,
};
use lifiswap::provider::{Provider, StepExecutor};
use lifiswap::types::{
    Chain, ExecutionError, ExecutionOptions, ExecutionStatus, InteractionSettings,
    LiFiStepExtended, StepExecutorOptions,
};

use crate::api::BlockchainApi;
use crate::errors::parse_bitcoin_error;
use crate::signer::BtcSigner;
use crate::tasks::{BtcConfirmTask, BtcSignTask};

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

        TaskPipeline::new(vec![
            Box::new(CheckBalanceTask),
            Box::new(PrepareTransactionTask),
            Box::new(BtcSignTask::new(Arc::clone(&self.signer), self.api.clone())),
            Box::new(BtcConfirmTask::new(self.api.clone())),
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

            let status_manager = StatusManager::new(
                self.options.route_id.clone(),
                client.execution_state().clone(),
            );
            status_manager.initialize_execution(step);

            let is_bridge = step.action.from_chain_id != step.action.to_chain_id;
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
                from_chain,
                signed_typed_data: vec![],
            };

            let result = pipeline.run(&mut ctx).await;

            if let Err(ref e) = result {
                let error_msg = e.to_string();
                let parsed = parse_bitcoin_error(&error_msg);
                let (code, message) = match &parsed {
                    LiFiError::Transaction { code, message } => {
                        ((*code as u16).to_string(), message.clone())
                    }
                    _ => ((LiFiErrorCode::InternalError as u16).to_string(), error_msg),
                };

                status_manager.update_execution(
                    ctx.step,
                    ExecutionUpdate {
                        status: Some(ExecutionStatus::Failed),
                        error: Some(ExecutionError {
                            code,
                            message,
                            html_message: None,
                        }),
                        ..Default::default()
                    },
                );
            }

            result.map(|_| ())
        })
    }

    fn set_interaction(&mut self, settings: InteractionSettings) {
        self.interaction = settings;
    }

    fn allow_execution(&self) -> bool {
        self.interaction.allow_execution
    }
}
