//! SVM step executor — orchestrates the Solana task pipeline.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

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
use solana_sdk::transaction::VersionedTransaction;

use crate::rpc::RpcPool;
use crate::signer::SvmSigner;
use crate::tasks::{SvmSendAndConfirmTask, SvmSignTask};

/// Solana step executor.
///
/// Builds a [`TaskPipeline`] with the following sequence:
///
/// 1. `CheckBalanceTask` — verify wallet has sufficient funds
/// 2. `PrepareTransactionTask` — fetch transaction data from API
/// 3. `SvmSignTask` — sign base64-encoded transaction(s)
/// 4. `SvmSendAndConfirmTask` — send to RPCs and poll confirmation
/// 5. `WaitForTransactionStatusTask` — poll `LiFi` status until terminal
pub struct SvmStepExecutor {
    signer: Arc<dyn SvmSigner>,
    rpc_pool: RpcPool,
    options: StepExecutorOptions,
    interaction: InteractionSettings,
    skip_simulation: bool,
}

impl std::fmt::Debug for SvmStepExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SvmStepExecutor")
            .field("pubkey", &self.signer.pubkey())
            .field("rpc_count", &self.rpc_pool.len())
            .field("options", &self.options)
            .field("interaction", &self.interaction)
            .finish_non_exhaustive()
    }
}

impl SvmStepExecutor {
    /// Create a new Solana step executor.
    pub(crate) fn new(
        signer: Arc<dyn SvmSigner>,
        rpc_pool: RpcPool,
        options: StepExecutorOptions,
        skip_simulation: bool,
    ) -> Self {
        Self {
            signer,
            rpc_pool,
            options,
            interaction: InteractionSettings::default(),
            skip_simulation,
        }
    }

    fn build_pipeline(&self, is_bridge: bool) -> TaskPipeline {
        let signed_txs: Arc<Mutex<Vec<VersionedTransaction>>> = Arc::new(Mutex::new(Vec::new()));

        let status_task = if is_bridge {
            WaitForTransactionStatusTask::receiving_chain()
        } else {
            WaitForTransactionStatusTask::swap()
        };

        TaskPipeline::new(vec![
            Box::new(CheckBalanceTask),
            Box::new(PrepareTransactionTask),
            Box::new(SvmSignTask::new(
                Arc::clone(&self.signer),
                Arc::clone(&signed_txs),
            )),
            Box::new(SvmSendAndConfirmTask::new(
                self.rpc_pool.clone(),
                self.skip_simulation,
                signed_txs,
            )),
            Box::new(status_task),
        ])
    }
}

impl StepExecutor for SvmStepExecutor {
    fn execute_step<'a>(
        &'a mut self,
        client: &'a LiFiClient,
        step: &'a mut LiFiStepExtended,
        provider: &'a dyn Provider,
        execution_options: &'a ExecutionOptions,
        from_chain: &'a Chain,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            // Verify signer pubkey matches the step's fromAddress
            if let Some(ref from_addr) = step.action.from_address {
                let expected = from_addr
                    .parse::<solana_sdk::pubkey::Pubkey>()
                    .map_err(|_| {
                        LiFiError::Validation(format!("Invalid Solana fromAddress: {from_addr}"))
                    })?;
                if expected != self.signer.pubkey() {
                    return Err(LiFiError::Transaction {
                        code: LiFiErrorCode::WalletChangedDuringExecution,
                        message: "The wallet address that requested the quote does not match \
                                  the wallet address attempting to sign the transaction."
                            .to_owned(),
                    });
                }
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
                signed_typed_data: Vec::new(),
            };

            let result = pipeline.run(&mut ctx).await;

            if let Err(err) = result {
                let parsed = crate::errors::parse_solana_error(err);

                if !matches!(parsed, LiFiError::StepRetry { .. }) {
                    let exec_error = error_to_execution_error(&parsed);
                    let last_action_type =
                        ctx.step.execution.as_ref().and_then(|e| e.last_action_type);

                    if let Some(action_type) = last_action_type {
                        let _ = status_manager.update_action(
                            ctx.step,
                            action_type,
                            lifiswap::types::ExecutionActionStatus::Failed,
                            Some(lifiswap::execution::status::ActionUpdateParams {
                                error: Some(exec_error),
                                ..Default::default()
                            }),
                        );
                    } else {
                        status_manager.update_execution(
                            ctx.step,
                            ExecutionUpdate {
                                status: Some(ExecutionStatus::Failed),
                                error: Some(exec_error),
                                ..Default::default()
                            },
                        );
                    }
                }

                return Err(parsed);
            }

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

fn error_to_execution_error(err: &LiFiError) -> ExecutionError {
    let code = match err {
        LiFiError::Transaction { code, .. } | LiFiError::Provider { code, .. } => code.to_string(),
        LiFiError::Http(details) => details.code.to_string(),
        LiFiError::Balance(_) => "1013".to_owned(),
        _ => "1000".to_owned(),
    };
    ExecutionError {
        code,
        message: err.to_string(),
        html_message: None,
    }
}
