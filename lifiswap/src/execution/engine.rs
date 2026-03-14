//! Core execution engine — route execution, resumption, and management.
//!
//! All execution functions are methods on [`LiFiClient`] so they
//! automatically share the client's [`ExecutionState`].

use crate::LiFiClient;
use crate::error::{LiFiError, LiFiErrorCode, Result};
use crate::provider::Provider;
use crate::types::{
    ExecutionOptions, ExecutionStatus, InteractionSettings, Route, RouteExtended,
    StepExecutorOptions,
};

impl LiFiClient {
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
    /// let route = client.get_routes(&request).await?.routes.into_iter().next().unwrap();
    /// let extended = client.execute_route(route, &providers, Default::default()).await?;
    /// ```
    pub async fn execute_route(
        &self,
        route: Route,
        providers: &[Box<dyn Provider>],
        options: ExecutionOptions,
    ) -> Result<RouteExtended> {
        let extended: RouteExtended = route.into();
        self.execute_steps(extended, providers, options).await
    }

    /// Resume a previously started (and possibly failed/paused) route.
    ///
    /// The route should contain execution state from a prior run.
    /// Steps that are already `Done` are skipped.
    ///
    /// # Errors
    ///
    /// Same as [`LiFiClient::execute_route`].
    pub async fn resume_route(
        &self,
        route: RouteExtended,
        providers: &[Box<dyn Provider>],
        options: ExecutionOptions,
    ) -> Result<RouteExtended> {
        self.execute_steps(route, providers, options).await
    }

    /// Stop execution of an active route.
    ///
    /// Sets all active step executors to disallow execution and removes the
    /// route from the execution state.
    pub fn stop_route_execution(&self, route_id: &str) {
        let state = self.execution_state();
        state.with_route(route_id, |data| {
            for executor in &mut data.executors {
                executor.set_interaction(InteractionSettings {
                    allow_interaction: false,
                    allow_updates: false,
                    allow_execution: false,
                });
            }
        });
        state.delete(route_id);
    }

    /// Get all active routes currently being executed.
    #[must_use]
    pub fn get_active_routes(&self) -> Vec<RouteExtended> {
        let state = self.execution_state();
        let ids = state.active_route_ids();
        ids.iter()
            .filter_map(|id| state.get(id).map(|d| d.route.clone()))
            .collect()
    }

    /// Get a specific active route by ID.
    #[must_use]
    pub fn get_active_route(&self, route_id: &str) -> Option<RouteExtended> {
        self.execution_state()
            .get(route_id)
            .map(|d| d.route.clone())
    }

    async fn execute_steps(
        &self,
        mut route: RouteExtended,
        providers: &[Box<dyn Provider>],
        options: ExecutionOptions,
    ) -> Result<RouteExtended> {
        if route.steps.is_empty() {
            return Err(LiFiError::Validation("Route has no steps.".to_owned()));
        }

        let state = self.execution_state();
        let execute_in_background = options.execute_in_background;
        state.create(route.clone(), options);

        // Prefetch chain list once instead of per-step
        let chains = self.get_chains(None).await?;

        for step_idx in 0..route.steps.len() {
            let step = &route.steps[step_idx];

            if let Some(ref exec) = step.execution
                && exec.status == ExecutionStatus::Done
            {
                tracing::debug!(step_id = %step.step.id, "skipping completed step");
                continue;
            }

            let from_chain_id = step.step.action.from_chain_id;

            let chain = chains
                .iter()
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
                "executing step"
            );

            executor.execute_step(self, step_ref).await?;

            state.with_route(&route.id, |data| {
                if step_idx < data.route.steps.len() {
                    data.route.steps[step_idx] = step_ref.clone();
                }
                data.executors.push(executor);
            });
        }

        let final_route = state
            .get(&route.id)
            .map(|d| d.route.clone())
            .unwrap_or(route);

        state.delete(&final_route.id);

        Ok(final_route)
    }
}
