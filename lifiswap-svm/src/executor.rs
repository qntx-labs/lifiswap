//! SVM step executor — orchestrates the Solana task pipeline.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use lifiswap::LiFiClient;
use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::execution::run::run_step_pipeline;
use lifiswap::execution::task::{ExecutionTask, TaskPipeline};
use lifiswap::execution::tasks::{
    CheckBalanceTask, PrepareTransactionTask, WaitForTransactionStatusTask,
};
use lifiswap::provider::{Provider, StepExecutor};
use lifiswap::types::{
    Chain, ExecutionOptions, InteractionSettings, LiFiStepExtended, StepExecutorOptions,
};
use solana_sdk::transaction::VersionedTransaction;

use crate::jito::JitoClient;
use crate::rpc::RpcPool;
use crate::signer::SvmSigner;
use crate::tasks::{SvmJitoSendAndConfirmTask, SvmSendAndConfirmTask, SvmSignTask};

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
    jito: Option<JitoClient>,
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
        jito: Option<JitoClient>,
    ) -> Self {
        Self {
            signer,
            rpc_pool,
            options,
            interaction: InteractionSettings::default(),
            skip_simulation,
            jito,
        }
    }

    fn build_pipeline(&self, is_bridge: bool) -> TaskPipeline {
        let signed_txs: Arc<Mutex<Vec<VersionedTransaction>>> = Arc::new(Mutex::new(Vec::new()));

        let status_task = if is_bridge {
            WaitForTransactionStatusTask::receiving_chain()
        } else {
            WaitForTransactionStatusTask::swap()
        };

        let sign_task = Box::new(SvmSignTask::new(
            Arc::clone(&self.signer),
            Arc::clone(&signed_txs),
        ));

        let send_confirm_task: Box<dyn ExecutionTask> = if let Some(ref jito) = self.jito {
            Box::new(SvmJitoSendAndConfirmTask::new(
                jito.clone(),
                Arc::clone(&signed_txs),
            ))
        } else {
            Box::new(SvmSendAndConfirmTask::new(
                self.rpc_pool.clone(),
                self.skip_simulation,
                signed_txs,
            ))
        };

        TaskPipeline::new(vec![
            Box::new(CheckBalanceTask),
            Box::new(PrepareTransactionTask),
            sign_task,
            send_confirm_task,
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
                crate::errors::parse_solana_error,
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
