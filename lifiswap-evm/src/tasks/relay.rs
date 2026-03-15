use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::execution::status::ActionUpdateParams;
use lifiswap::execution::task::{ExecutionContext, ExecutionTask};
use lifiswap::types::{
    ExecutionActionStatus, ExecutionActionType, TaskStatus, TransactionMethodType,
};

use super::{get_domain_chain_id, now_ms};
use crate::signer::EvmSigner;

fn serialize_typed_data_entry<T: serde::Serialize>(
    typed_data: &T,
    signature: Option<&str>,
) -> Result<serde_json::Value> {
    let mut entry = serde_json::to_value(typed_data).map_err(|e| LiFiError::Transaction {
        code: LiFiErrorCode::InternalError,
        message: format!("Failed to serialize typed data: {e}"),
    })?;
    if let (serde_json::Value::Object(map), Some(sig)) = (&mut entry, signature) {
        map.insert(
            "signature".to_owned(),
            serde_json::Value::String(sig.to_owned()),
        );
    }
    Ok(entry)
}

/// Sign EIP-712 typed data and relay via the LI.FI relayer (gasless).
///
/// Flow:
/// 1. Extract unsigned `typed_data` from the step
/// 2. Sign each entry via [`EvmSigner::sign_typed_data`]
/// 3. Submit signed data to `client.relay_transaction()`
/// 4. Update action with `task_id` for status polling
pub struct EvmRelaySignAndExecuteTask {
    signer: Arc<dyn EvmSigner>,
}

impl std::fmt::Debug for EvmRelaySignAndExecuteTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvmRelaySignAndExecuteTask")
            .field("address", &self.signer.address())
            .finish_non_exhaustive()
    }
}

impl EvmRelaySignAndExecuteTask {
    pub fn new(signer: Arc<dyn EvmSigner>) -> Self {
        Self { signer }
    }
}

impl ExecutionTask for EvmRelaySignAndExecuteTask {
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
            let all_typed_data = ctx
                .step
                .typed_data
                .as_ref()
                .ok_or_else(|| LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "No typed data found for relay transaction.".to_owned(),
                })?
                .clone();

            if all_typed_data.is_empty() {
                return Err(LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "Typed data array is empty.".to_owned(),
                });
            }

            let action_type = if ctx.is_bridge_execution {
                ExecutionActionType::CrossChain
            } else {
                ExecutionActionType::Swap
            };

            // Filter out typed data entries that have already been signed
            let intent_typed_data: Vec<_> = all_typed_data
                .iter()
                .filter(|td| {
                    !ctx.signed_typed_data.iter().any(|signed| {
                        signed.typed_data.as_ref().is_some_and(|std| {
                            std.primary_type == td.primary_type
                                && std.domain.as_ref().and_then(get_domain_chain_id)
                                    == td.domain.as_ref().and_then(get_domain_chain_id)
                        })
                    })
                })
                .collect();

            if intent_typed_data.is_empty() {
                return Err(LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "Typed data for transfer is not found after filtering permits."
                        .to_owned(),
                });
            }

            ctx.status_manager.update_action(
                ctx.step,
                action_type,
                ExecutionActionStatus::MessageRequired,
                None,
            )?;

            if !ctx.allow_user_interaction {
                return Ok(TaskStatus::Paused);
            }

            // Check for Hyperliquid agent step
            let is_hyperliquid = ctx
                .step
                .tool
                .as_deref()
                .is_some_and(|t| t == "hyperliquidSpotProtocol")
                && intent_typed_data.iter().any(|td| {
                    td.primary_type.as_deref() == Some("HyperliquidTransaction:ApproveAgent")
                });

            // Start with already-signed permit data
            let mut signed_data: Vec<serde_json::Value> = Vec::with_capacity(all_typed_data.len());

            for signed in &ctx.signed_typed_data {
                if let Some(ref td) = signed.typed_data {
                    signed_data.push(serialize_typed_data_entry(td, signed.signature.as_deref())?);
                }
            }

            if is_hyperliquid {
                // Hyperliquid agent wallet signing via hook
                let hook = ctx
                    .execution_options
                    .sign_hyperliquid
                    .as_ref()
                    .ok_or_else(|| LiFiError::Transaction {
                        code: LiFiErrorCode::InternalError,
                        message: "Hyperliquid agent step requires sign_hyperliquid hook."
                            .to_owned(),
                    })?;

                let hl_results = hook(lifiswap::types::HyperliquidSignParams {
                    tool: ctx.step.tool.clone().unwrap_or_default(),
                    owner_address: format!("{:#x}", self.signer.address()),
                    typed_data: intent_typed_data.into_iter().cloned().collect(),
                })
                .await;

                for signed in &hl_results {
                    if let Some(ref td) = signed.typed_data {
                        signed_data
                            .push(serialize_typed_data_entry(td, signed.signature.as_deref())?);
                    }
                }
            } else {
                // Standard relay signing with chain switch support
                let from_chain_id = ctx.step.action.from_chain_id.0;
                for td in &intent_typed_data {
                    let target_chain_id = td
                        .domain
                        .as_ref()
                        .and_then(get_domain_chain_id)
                        .unwrap_or(from_chain_id);
                    if target_chain_id != from_chain_id {
                        self.signer.switch_chain(target_chain_id).await?;
                    }

                    let signature = self.signer.sign_typed_data(td).await?;
                    signed_data.push(serialize_typed_data_entry(td, Some(&signature))?);
                }

                // Switch back if needed
                if intent_typed_data.iter().any(|td| {
                    td.domain
                        .as_ref()
                        .and_then(get_domain_chain_id)
                        .is_some_and(|id| id != from_chain_id)
                }) {
                    self.signer.switch_chain(from_chain_id).await?;
                }
            }

            ctx.status_manager.update_action(
                ctx.step,
                action_type,
                ExecutionActionStatus::Pending,
                None,
            )?;

            let relay_resp = ctx
                .client
                .relay_transaction(&lifiswap::types::RelayRequest {
                    typed_data: signed_data,
                })
                .await?;

            ctx.status_manager.update_action(
                ctx.step,
                action_type,
                ExecutionActionStatus::Pending,
                Some(ActionUpdateParams {
                    task_id: relay_resp.task_id.clone(),
                    tx_type: Some(TransactionMethodType::Relayed),
                    signed_at: Some(now_ms()),
                    tx_link: relay_resp.tx_link.clone(),
                    ..Default::default()
                }),
            )?;

            tracing::info!(
                task_id = ?relay_resp.task_id,
                "relay transaction submitted"
            );

            if ctx.is_bridge_execution {
                ctx.status_manager.update_action(
                    ctx.step,
                    action_type,
                    ExecutionActionStatus::Done,
                    None,
                )?;
            }

            Ok(TaskStatus::Completed)
        })
    }
}
