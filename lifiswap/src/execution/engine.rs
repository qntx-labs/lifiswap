//! Core execution engine — `execute_route`, `resume_route`, `stop_route_execution`.

use super::state::EXECUTION_STATE;
use crate::LiFiClient;
use crate::error::{LiFiError, LiFiErrorCode, Result};
use crate::provider::Provider;
use crate::types::{
    ExecutionOptions, ExecutionStatus, InteractionSettings, Route, RouteExtended,
    StepExecutorOptions,
};

/// Execute a route from start to finish.
///
/// Converts the [`Route`] into a [`RouteExtended`] with execution tracking,
/// then executes each step sequentially using the appropriate chain provider.
///
/// # Errors
///
/// Returns an error if:
/// - No provider is registered for a step's chain type
/// - Any step execution fails
/// - The route has no steps
///
/// # Example
///
/// ```ignore
/// use lifiswap::execution::execute_route;
///
/// let route = client.get_routes(&request).await?.routes.into_iter().next().unwrap();
/// let extended = execute_route(&client, route, &providers, Default::default()).await?;
/// ```
pub async fn execute_route(
    client: &LiFiClient,
    route: Route,
    providers: &[Box<dyn Provider>],
    options: ExecutionOptions,
) -> Result<RouteExtended> {
    let extended: RouteExtended = route.into();
    execute_steps(client, extended, providers, options).await
}

/// Resume a previously started (and possibly failed/paused) route.
///
/// The route should contain execution state from a prior run.
/// Steps that are already `Done` are skipped.
///
/// # Errors
///
/// Same as [`execute_route`].
pub async fn resume_route(
    client: &LiFiClient,
    route: RouteExtended,
    providers: &[Box<dyn Provider>],
    options: ExecutionOptions,
) -> Result<RouteExtended> {
    execute_steps(client, route, providers, options).await
}

/// Stop execution of an active route.
///
/// Sets all active step executors to disallow execution and removes the
/// route from the global execution state.
pub fn stop_route_execution(route_id: &str) {
    EXECUTION_STATE.with_route(route_id, |data| {
        for executor in &mut data.executors {
            executor.set_interaction(InteractionSettings {
                allow_interaction: false,
                allow_updates: false,
                allow_execution: false,
            });
        }
    });
    EXECUTION_STATE.delete(route_id);
}

/// Get all active route IDs and their current extended routes.
#[must_use]
pub fn get_active_routes() -> Vec<RouteExtended> {
    let ids = EXECUTION_STATE.active_route_ids();
    ids.iter()
        .filter_map(|id| EXECUTION_STATE.get(id).map(|d| d.route.clone()))
        .collect()
}

/// Get a specific active route by ID.
#[must_use]
pub fn get_active_route(route_id: &str) -> Option<RouteExtended> {
    EXECUTION_STATE.get(route_id).map(|d| d.route.clone())
}

async fn execute_steps(
    client: &LiFiClient,
    mut route: RouteExtended,
    providers: &[Box<dyn Provider>],
    options: ExecutionOptions,
) -> Result<RouteExtended> {
    if route.steps.is_empty() {
        return Err(LiFiError::Validation("Route has no steps.".to_owned()));
    }

    let execute_in_background = options.execute_in_background;
    EXECUTION_STATE.create(route.clone(), options);

    for step_idx in 0..route.steps.len() {
        let step = &route.steps[step_idx];

        if let Some(ref exec) = step.execution
            && exec.status == ExecutionStatus::Done
        {
            tracing::debug!(step_id = %step.step.id, "skipping completed step");
            continue;
        }

        let from_chain_id = step.step.action.from_chain_id;
        let to_chain_id = step.step.action.to_chain_id;
        let is_bridge = from_chain_id != to_chain_id;

        let chain = client
            .get_chains(None)
            .await?
            .into_iter()
            .find(|c| c.id == from_chain_id)
            .ok_or_else(|| LiFiError::Provider {
                code: LiFiErrorCode::ProviderUnavailable,
                message: format!("No chain info found for chain ID {from_chain_id:?}"),
            })?;

        let chain_type = chain.chain_type;

        let provider = providers
            .iter()
            .find(|p| p.chain_type() == chain_type)
            .ok_or_else(|| LiFiError::Provider {
                code: LiFiErrorCode::ProviderUnavailable,
                message: format!("No provider registered for chain type {chain_type:?}"),
            })?;

        let mut executor = provider
            .create_step_executor(StepExecutorOptions {
                route_id: route.id.clone(),
                execute_in_background,
            })
            .await?;

        if execute_in_background {
            executor.set_interaction(InteractionSettings {
                allow_interaction: false,
                allow_updates: true,
                allow_execution: true,
            });
        }

        let step_ref = &mut route.steps[step_idx];

        tracing::info!(
            step_id = %step_ref.step.id,
            step_type = %step_ref.step.step_type,
            is_bridge,
            "executing step"
        );

        executor.execute_step(client, step_ref).await?;

        EXECUTION_STATE.with_route(&route.id, |data| {
            if step_idx < data.route.steps.len() {
                data.route.steps[step_idx] = step_ref.clone();
            }
            data.executors.push(executor);
        });
    }

    let final_route = EXECUTION_STATE
        .get(&route.id)
        .map(|d| d.route.clone())
        .unwrap_or(route);

    EXECUTION_STATE.delete(&final_route.id);

    Ok(final_route)
}
