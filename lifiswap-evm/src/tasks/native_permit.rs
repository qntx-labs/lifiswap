use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use lifiswap::error::Result;
use lifiswap::execution::task::{ExecutionContext, ExecutionTask};
use lifiswap::types::{ExecutionActionStatus, ExecutionActionType, TaskStatus};

use super::get_domain_chain_id;
use crate::executor::Permit2Config;
use crate::is_native_token;
use crate::signer::EvmSigner;

/// Obtain and sign an EIP-2612 native permit if available.
///
/// Skips when:
/// - The token is native (ETH)
/// - `skipPermit` is set
/// - No `permit2_proxy` is configured
/// - Batching is active (approve is included in the batch)
/// - A matching permit already exists in `signed_typed_data`
/// - The `get_native_permit` hook is not configured
///
/// When a native permit is obtained and signed, it is stored in
/// [`ExecutionContext::signed_typed_data`] for downstream tasks to wrap
/// the transaction calldata.
pub struct EvmNativePermitTask {
    signer: Arc<dyn EvmSigner>,
    permit2: Option<Permit2Config>,
}

impl std::fmt::Debug for EvmNativePermitTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvmNativePermitTask")
            .field("address", &self.signer.address())
            .finish_non_exhaustive()
    }
}

impl EvmNativePermitTask {
    pub fn new(signer: Arc<dyn EvmSigner>, permit2: Option<Permit2Config>) -> Self {
        Self { signer, permit2 }
    }
}

impl ExecutionTask for EvmNativePermitTask {
    fn should_run<'a>(
        &'a self,
        ctx: &'a ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            if is_native_token(&ctx.step.action.from_token.address) {
                return false;
            }
            if ctx
                .step
                .estimate
                .as_ref()
                .and_then(|e| e.skip_permit)
                .unwrap_or(false)
            {
                return false;
            }
            // Need permit2_proxy configured
            if self.permit2.is_none() {
                return false;
            }
            // Skip if batching (approve is in the batch)
            if self.signer.supports_batching() {
                return false;
            }
            // Skip if hook is not configured
            if ctx.execution_options.get_native_permit.is_none() {
                return false;
            }
            // Skip if already have a matching permit
            let from_chain_id = ctx.step.action.from_chain_id.0;
            let has_matching = ctx.signed_typed_data.iter().any(|s| {
                s.typed_data
                    .as_ref()
                    .and_then(|td| td.domain.as_ref())
                    .and_then(get_domain_chain_id)
                    .is_some_and(|id| id == from_chain_id)
            });
            !has_matching
        })
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = Result<TaskStatus>> + Send + 'a>> {
        Box::pin(async move {
            let from_chain_id = ctx.step.action.from_chain_id.0;
            let permit2_proxy = self.permit2.unwrap().permit2_proxy;

            ctx.status_manager.initialize_action(
                ctx.step,
                ExecutionActionType::NativePermit,
                from_chain_id,
                ExecutionActionStatus::Started,
            )?;

            let hook = ctx.execution_options.get_native_permit.as_ref().unwrap();

            let permit_data = hook(lifiswap::types::NativePermitParams {
                chain_id: ctx.step.action.from_chain_id,
                token_address: ctx.step.action.from_token.address.clone(),
                spender_address: format!("{permit2_proxy:#x}"),
                owner_address: ctx.step.action.from_address.clone().unwrap_or_default(),
                amount: ctx.step.action.from_amount.clone().unwrap_or_default(),
            })
            .await;

            let Some(typed_data) = permit_data else {
                ctx.status_manager.update_action(
                    ctx.step,
                    ExecutionActionType::NativePermit,
                    ExecutionActionStatus::Done,
                    None,
                )?;
                return Ok(TaskStatus::Completed);
            };

            ctx.status_manager.update_action(
                ctx.step,
                ExecutionActionType::NativePermit,
                ExecutionActionStatus::ActionRequired,
                None,
            )?;

            if !ctx.allow_user_interaction {
                return Ok(TaskStatus::Paused);
            }

            let signature = self.signer.sign_typed_data(&typed_data).await?;

            ctx.signed_typed_data
                .push(lifiswap::types::SignedTypedData {
                    typed_data: Some(typed_data),
                    signature: Some(signature),
                });

            ctx.status_manager.update_action(
                ctx.step,
                ExecutionActionType::NativePermit,
                ExecutionActionStatus::Done,
                None,
            )?;

            Ok(TaskStatus::Completed)
        })
    }
}
