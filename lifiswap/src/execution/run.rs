//! Shared step execution logic for all chain providers.
//!
//! Extracts the common `execute_step` boilerplate (context creation,
//! pipeline running, error handling) that was duplicated across EVM,
//! SVM, and BTC executors.

use crate::LiFiClient;
use crate::error::{LiFiError, LiFiErrorCode, Result};
use crate::execution::status::{ActionUpdateParams, ExecutionUpdate, StatusManager};
use crate::execution::task::{ExecutionContext, TaskPipeline};
use crate::provider::Provider;
use crate::types::{
    Chain, ExecutionActionStatus, ExecutionError, ExecutionOptions, ExecutionStatus,
    LiFiStepExtended, StepExecutorOptions,
};

/// Convert a [`LiFiError`] into an [`ExecutionError`] for status tracking.
#[must_use]
pub fn error_to_execution_error(err: &LiFiError) -> ExecutionError {
    let code = match err {
        LiFiError::Transaction { code, .. } | LiFiError::Provider { code, .. } => code.to_string(),
        LiFiError::Http(details) => details.code.to_string(),
        LiFiError::Balance(_) => (LiFiErrorCode::BalanceError as u16).to_string(),
        _ => (LiFiErrorCode::InternalError as u16).to_string(),
    };
    ExecutionError {
        code,
        message: err.to_string(),
        html_message: None,
    }
}

/// Run the common step execution flow shared by all chain providers.
///
/// Handles:
/// 1. `StatusManager` creation and execution initialization
/// 2. `ExecutionContext` construction
/// 3. Pipeline execution
/// 4. Error handling — updates action or execution status on failure,
///    matching the TS SDK's `BaseStepExecutor.executeStep` pattern
///
/// Chain-specific executors only need to:
/// - Verify the signer address
/// - Build the pipeline
/// - Provide a `parse_error` function
///
/// # Errors
///
/// Returns the (possibly parsed) error from the pipeline. On failure,
/// the step's execution or action status is also updated.
#[allow(clippy::too_many_arguments)]
pub async fn run_step_pipeline(
    client: &LiFiClient,
    step: &mut LiFiStepExtended,
    provider: &dyn Provider,
    execution_options: &ExecutionOptions,
    from_chain: &Chain,
    options: &StepExecutorOptions,
    allow_user_interaction: bool,
    pipeline: TaskPipeline,
    parse_error: impl FnOnce(LiFiError) -> LiFiError,
) -> Result<()> {
    let status_manager =
        StatusManager::new(options.route_id.clone(), client.execution_state().clone());
    status_manager.initialize_execution(step);

    let is_bridge = step.action.from_chain_id != step.action.to_chain_id;

    let mut ctx = ExecutionContext {
        client,
        step,
        status_manager: &status_manager,
        provider,
        route_id: &options.route_id,
        execution_options,
        is_bridge_execution: is_bridge,
        allow_user_interaction,
        from_chain,
        signed_typed_data: Vec::new(),
    };

    let result = pipeline.run(&mut ctx).await;

    if let Err(err) = result {
        let parsed = parse_error(err);

        if !matches!(parsed, LiFiError::StepRetry { .. }) {
            let exec_error = error_to_execution_error(&parsed);
            let last_action_type = ctx.step.execution.as_ref().and_then(|e| e.last_action_type);

            if let Some(action_type) = last_action_type {
                let _ = status_manager.update_action(
                    ctx.step,
                    action_type,
                    ExecutionActionStatus::Failed,
                    Some(ActionUpdateParams {
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
}
