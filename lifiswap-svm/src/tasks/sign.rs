//! Sign task — deserializes and signs Solana transactions from the API response.
//!
//! Mirrors the TS SDK's `SolanaSignAndExecuteTask`: decodes base64-encoded
//! transaction(s) from the step's `transaction_request.data`, signs them via
//! the [`SvmSigner`], and stores the signed bytes for the send task.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::execution::status::ActionUpdateParams;
use lifiswap::execution::task::{ExecutionContext, ExecutionTask};
use lifiswap::types::{ExecutionActionStatus, ExecutionActionType, TaskStatus};
use solana_sdk::transaction::VersionedTransaction;

use super::now_ms;
use crate::signer::SvmSigner;

/// Signs Solana transaction(s) from the step's `transaction_request.data`.
///
/// The API may return:
/// - A single base64-encoded transaction string
/// - A JSON array of base64-encoded transaction strings
///
/// Signed transaction bytes are stored in the shared `signed_txs` mutex
/// for consumption by [`SvmSendAndConfirmTask`](super::SvmSendAndConfirmTask).
pub struct SvmSignTask {
    signer: Arc<dyn SvmSigner>,
    signed_txs: Arc<Mutex<Vec<VersionedTransaction>>>,
}

impl std::fmt::Debug for SvmSignTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SvmSignTask")
            .field("pubkey", &self.signer.pubkey())
            .finish_non_exhaustive()
    }
}

impl SvmSignTask {
    pub(crate) fn new(
        signer: Arc<dyn SvmSigner>,
        signed_txs: Arc<Mutex<Vec<VersionedTransaction>>>,
    ) -> Self {
        Self { signer, signed_txs }
    }
}

/// Parse transaction data from the step. The API returns either a single
/// base64 string or a JSON array of base64 strings in `transaction_request.data`.
fn parse_transaction_data(data: &str) -> Result<Vec<Vec<u8>>> {
    let trimmed = data.trim();

    let b64_strings: Vec<String> = if trimmed.starts_with('[') {
        serde_json::from_str(trimmed).map_err(|e| LiFiError::Transaction {
            code: LiFiErrorCode::InternalError,
            message: format!("Failed to parse transaction data array: {e}"),
        })?
    } else {
        vec![trimmed.to_owned()]
    };

    b64_strings
        .iter()
        .map(|s| {
            base64::engine::general_purpose::STANDARD
                .decode(s)
                .map_err(|e| LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: format!("Invalid base64 transaction data: {e}"),
                })
        })
        .collect()
}

impl ExecutionTask for SvmSignTask {
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

            let _action = ctx
                .status_manager
                .find_action(ctx.step, action_type)
                .ok_or(LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "Unable to prepare transaction. Action not found.".to_owned(),
                })?;

            let tx_data = ctx
                .step
                .transaction_request
                .as_ref()
                .and_then(|r| r.data.as_deref())
                .ok_or_else(|| LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "No transaction request data available.".to_owned(),
                })?;

            let tx_bytes_list = parse_transaction_data(tx_data)?;

            let transactions: Vec<VersionedTransaction> = tx_bytes_list
                .iter()
                .map(|bytes| {
                    bincode::deserialize(bytes).map_err(|e| LiFiError::Transaction {
                        code: LiFiErrorCode::InternalError,
                        message: format!("Failed to deserialize Solana transaction: {e}"),
                    })
                })
                .collect::<Result<Vec<_>>>()?;

            if transactions.is_empty() {
                return Err(LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "No transactions to sign.".to_owned(),
                });
            }

            let timeout = tokio::time::Duration::from_secs(120);
            let signed = tokio::time::timeout(timeout, self.signer.sign_transactions(transactions))
                .await
                .map_err(|_| LiFiError::Transaction {
                    code: LiFiErrorCode::TransactionExpired,
                    message: "Transaction signing timed out: blockhash may no longer be valid."
                        .to_owned(),
                })??;

            if signed.is_empty() {
                return Err(LiFiError::Transaction {
                    code: LiFiErrorCode::TransactionFailed,
                    message: "No signed transactions returned from signer.".to_owned(),
                });
            }

            ctx.status_manager.update_action(
                ctx.step,
                action_type,
                ExecutionActionStatus::Pending,
                Some(ActionUpdateParams {
                    signed_at: Some(now_ms()),
                    ..Default::default()
                }),
            )?;

            {
                let mut guard = self.signed_txs.lock().expect("signed_txs mutex poisoned");
                *guard = signed;
            }

            Ok(TaskStatus::Completed)
        })
    }
}
