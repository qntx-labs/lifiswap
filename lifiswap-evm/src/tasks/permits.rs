use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use lifiswap::error::Result;
use lifiswap::execution::task::{ExecutionContext, ExecutionTask};
use lifiswap::types::{ExecutionActionStatus, ExecutionActionType, TaskStatus};

use super::get_domain_chain_id;
use crate::signer::EvmSigner;

/// Sign any `Permit` typed data entries from the step before execution.
///
/// Filters `step.typedData` for entries with `primaryType == "Permit"`,
/// switches chain if the permit's EIP-712 domain specifies a different chain,
/// signs each one via [`EvmSigner::sign_typed_data`], and stores the
/// results in [`ExecutionContext::signed_typed_data`] for downstream tasks.
pub struct EvmCheckPermitsTask {
    signer: Arc<dyn EvmSigner>,
}

impl std::fmt::Debug for EvmCheckPermitsTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvmCheckPermitsTask")
            .field("address", &self.signer.address())
            .finish_non_exhaustive()
    }
}

impl EvmCheckPermitsTask {
    pub fn new(signer: Arc<dyn EvmSigner>) -> Self {
        Self { signer }
    }
}

impl ExecutionTask for EvmCheckPermitsTask {
    fn should_run<'a>(
        &'a self,
        ctx: &'a ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            ctx.step.typed_data.as_ref().is_some_and(|tds| {
                tds.iter()
                    .any(|td| td.primary_type.as_deref() == Some("Permit"))
            })
        })
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = Result<TaskStatus>> + Send + 'a>> {
        Box::pin(async move {
            let from_chain_id = ctx.step.action.from_chain_id.0;

            ctx.status_manager.initialize_action(
                ctx.step,
                ExecutionActionType::Permit,
                from_chain_id,
                ExecutionActionStatus::Started,
            )?;

            let permit_entries: Vec<_> = ctx
                .step
                .typed_data
                .as_ref()
                .map(|tds| {
                    tds.iter()
                        .filter(|td| td.primary_type.as_deref() == Some("Permit"))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();

            for td in &permit_entries {
                ctx.status_manager.update_action(
                    ctx.step,
                    ExecutionActionType::Permit,
                    ExecutionActionStatus::ActionRequired,
                    None,
                )?;

                if !ctx.allow_user_interaction {
                    return Ok(TaskStatus::Paused);
                }

                // Switch chain if the permit's domain specifies a different chain
                let target_chain_id = td
                    .domain
                    .as_ref()
                    .and_then(get_domain_chain_id)
                    .unwrap_or(from_chain_id);
                if target_chain_id != from_chain_id {
                    self.signer.switch_chain(target_chain_id).await?;
                }

                let signature = self.signer.sign_typed_data(td).await?;

                ctx.signed_typed_data
                    .push(lifiswap::types::SignedTypedData {
                        typed_data: Some(td.clone()),
                        signature: Some(signature),
                    });
            }

            // Switch back to the source chain after signing permits
            if permit_entries.iter().any(|td| {
                td.domain
                    .as_ref()
                    .and_then(get_domain_chain_id)
                    .is_some_and(|id| id != from_chain_id)
            }) {
                self.signer.switch_chain(from_chain_id).await?;
            }

            ctx.status_manager.update_action(
                ctx.step,
                ExecutionActionType::Permit,
                ExecutionActionStatus::Done,
                None,
            )?;

            Ok(TaskStatus::Completed)
        })
    }
}
