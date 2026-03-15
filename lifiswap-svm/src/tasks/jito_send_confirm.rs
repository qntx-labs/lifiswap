//! Jito bundle send and confirm task.
//!
//! Mirrors the TS SDK's `SolanaJitoWaitForTransactionTask`: submits
//! signed transactions as a Jito bundle and polls for bundle confirmation.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::execution::status::ActionUpdateParams;
use lifiswap::execution::task::{ExecutionContext, ExecutionTask};
use lifiswap::types::{ExecutionActionStatus, ExecutionActionType, TaskStatus};
use solana_sdk::transaction::VersionedTransaction;

use super::{get_tx_link, now_ms};
use crate::jito::JitoClient;

/// Sends signed Solana transactions as a Jito bundle and waits for confirmation.
///
/// This task is used when Jito bundle submission is enabled. It serializes
/// the signed transactions to base64, submits them as a bundle via the
/// Jito Block Engine, and polls for bundle confirmation.
pub struct SvmJitoSendAndConfirmTask {
    jito: JitoClient,
    signed_txs: Arc<Mutex<Vec<VersionedTransaction>>>,
}

impl std::fmt::Debug for SvmJitoSendAndConfirmTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SvmJitoSendAndConfirmTask")
            .field("jito", &self.jito)
            .finish_non_exhaustive()
    }
}

impl SvmJitoSendAndConfirmTask {
    /// Create a new Jito send-and-confirm task.
    pub(crate) const fn new(
        jito: JitoClient,
        signed_txs: Arc<Mutex<Vec<VersionedTransaction>>>,
    ) -> Self {
        Self { jito, signed_txs }
    }
}

impl ExecutionTask for SvmJitoSendAndConfirmTask {
    fn should_run<'a>(
        &'a self,
        ctx: &'a ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move { !ctx.has_committed_transaction() })
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = Result<TaskStatus>> + Send + 'a>> {
        Box::pin(async move {
            let action_type = if ctx.is_bridge_execution {
                ExecutionActionType::CrossChain
            } else {
                ExecutionActionType::Swap
            };

            let signed_txs = {
                let guard = self.signed_txs.lock().expect("signed_txs lock");
                guard.clone()
            };

            if signed_txs.is_empty() {
                return Err(LiFiError::Transaction {
                    code: LiFiErrorCode::TransactionUnprepared,
                    message: "No signed transactions available for Jito bundle.".to_owned(),
                });
            }

            // Serialize transactions to base64
            let base64_txs: Vec<String> = signed_txs
                .iter()
                .map(|tx| {
                    let bytes = bincode::serialize(tx).expect("VersionedTransaction serialization");
                    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes)
                })
                .collect();

            tracing::info!(tx_count = base64_txs.len(), "Submitting Jito bundle");

            let bundle_result = self.jito.send_and_confirm_bundle(&base64_txs).await?;

            // Use the first transaction signature for status tracking
            let tx_sig =
                bundle_result
                    .tx_signatures
                    .first()
                    .ok_or_else(|| LiFiError::Transaction {
                        code: LiFiErrorCode::TransactionFailed,
                        message: "Jito bundle confirmed but no transaction signatures found."
                            .to_owned(),
                    })?;

            let tx_sig_str = tx_sig.to_string();
            let tx_link = get_tx_link(ctx.from_chain, &tx_sig_str);

            tracing::info!(
                bundle_id = bundle_result.bundle_id.as_str(),
                tx_sig = tx_sig_str.as_str(),
                "Jito bundle confirmed"
            );

            ctx.status_manager.update_action(
                ctx.step,
                action_type,
                ExecutionActionStatus::Pending,
                Some(ActionUpdateParams {
                    tx_hash: Some(tx_sig_str),
                    tx_link,
                    signed_at: Some(now_ms()),
                    ..Default::default()
                }),
            )?;

            Ok(TaskStatus::Completed)
        })
    }
}
