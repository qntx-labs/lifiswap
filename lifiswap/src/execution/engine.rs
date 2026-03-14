//! Core execution engine — route execution, resumption, and management.
//!
//! All execution functions are methods on [`LiFiClient`] so they
//! automatically share the client's [`ExecutionState`].

use crate::LiFiClient;
use crate::error::{LiFiError, LiFiErrorCode, Result};
use crate::execution::convert::convert_quote_to_route;
use crate::provider::{Provider, StepExecutor};
use crate::types::{
    ExecutionOptions, ExecutionStatus, InteractionSettings, LiFiStep, QuoteRequest, Route,
    RouteExtended, StepExecutorOptions,
};

async fn create_executor(
    provider: &dyn Provider,
    route_id: String,
    execute_in_background: bool,
) -> Result<Box<dyn StepExecutor>> {
    let mut executor = provider
        .create_step_executor(StepExecutorOptions {
            route_id,
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
    Ok(executor)
}

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
    /// let extended = client.execute_route(route, Default::default()).await?;
    /// ```
    pub async fn execute_route(
        &self,
        route: Route,
        options: ExecutionOptions,
    ) -> Result<RouteExtended> {
        let extended: RouteExtended = route.into();
        self.execute_steps(extended, options).await
    }

    /// Execute a quote (single-step) by converting it to a route first.
    ///
    /// This is a convenience wrapper around [`convert_quote_to_route`] +
    /// [`execute_route`](Self::execute_route).
    ///
    /// # Errors
    ///
    /// Returns an error if the quote cannot be converted or execution fails.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let quote = client.get_quote(&request).await?;
    /// let result = client.execute_quote(quote, Default::default()).await?;
    /// ```
    pub async fn execute_quote(
        &self,
        quote: LiFiStep,
        options: ExecutionOptions,
    ) -> Result<RouteExtended> {
        let route = convert_quote_to_route(&quote, None)?;
        self.execute_route(route, options).await
    }

    /// Get the best quote and execute it in one call.
    ///
    /// This is the simplest way to perform a cross-chain or same-chain swap:
    /// a single method call that fetches the optimal quote from the LI.FI API,
    /// converts it into a route, and executes it end-to-end.
    ///
    /// # Errors
    ///
    /// Returns an error if the quote request fails, conversion fails,
    /// or execution fails.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use lifiswap::types::QuoteRequest;
    ///
    /// let result = client
    ///     .swap(
    ///         &QuoteRequest::builder()
    ///             .from_chain("42161")
    ///             .from_token("0xaf88d065e77c8cC2239327C5EDb3A432268e5831")
    ///             .from_address("0xYourWallet")
    ///             .from_amount("10000000")
    ///             .to_chain("10")
    ///             .to_token("0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1")
    ///             .build(),
    ///         Default::default(),
    ///     )
    ///     .await?;
    /// ```
    pub async fn swap(
        &self,
        request: &QuoteRequest,
        options: ExecutionOptions,
    ) -> Result<RouteExtended> {
        let quote = self.get_quote(request).await?;
        self.execute_quote(quote, options).await
    }

    /// Resume a previously started (and possibly failed/paused) route.
    ///
    /// Mirrors the `TypeScript` SDK's `resumeRoute` logic:
    /// 1. If the route is still actively executing and not halted,
    ///    updates execution options and returns the current route state.
    /// 2. Otherwise, calls [`prepare_restart`] to clean up stale actions
    ///    and re-executes from the point of failure.
    ///
    /// # Errors
    ///
    /// Same as [`LiFiClient::execute_route`].
    pub async fn resume_route(
        &self,
        mut route: RouteExtended,
        options: ExecutionOptions,
    ) -> Result<RouteExtended> {
        let state = self.execution_state();

        if let Some(data) = state.get(&route.id) {
            let execution_halted = data.executors.iter().any(|e| !e.allow_execution());

            if !execution_halted {
                drop(data);
                self.update_route_execution(&route.id, options);
                return self.get_active_route(&route.id).ok_or_else(|| {
                    LiFiError::Execution("Route execution promise not found.".to_owned())
                });
            }
            drop(data);
        }

        crate::execution::prepare_restart(&mut route);
        self.execute_steps(route, options).await
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

    /// Update execution settings for an active route.
    ///
    /// Primarily used to switch between foreground and background execution
    /// while a route is actively being processed. When `execute_in_background`
    /// is set, user interaction is disabled on all active executors.
    ///
    /// Does nothing if the route is not currently executing.
    pub fn update_route_execution(&self, route_id: &str, options: ExecutionOptions) {
        let state = self.execution_state();
        state.with_route(route_id, |data| {
            if options.execute_in_background {
                for executor in &mut data.executors {
                    executor.set_interaction(InteractionSettings {
                        allow_interaction: false,
                        allow_updates: true,
                        allow_execution: true,
                    });
                }
            } else {
                for executor in &mut data.executors {
                    executor.set_interaction(InteractionSettings::default());
                }
            }
            data.execution_options = options;
        });
    }

    async fn execute_steps(
        &self,
        mut route: RouteExtended,
        options: ExecutionOptions,
    ) -> Result<RouteExtended> {
        if route.steps.is_empty() {
            return Err(LiFiError::Validation("Route has no steps.".to_owned()));
        }

        let state = self.execution_state();
        let execute_in_background = options.execute_in_background;
        state.create(route.clone(), options);

        let chains = self.get_chains(None).await?;

        let result = self
            .execute_steps_inner(&mut route, &chains, execute_in_background)
            .await;

        match result {
            Ok(()) => {
                let final_route = state
                    .get(&route.id)
                    .map(|d| d.route.clone())
                    .unwrap_or(route);
                state.delete(&final_route.id);
                Ok(final_route)
            }
            Err(e) => {
                self.stop_route_execution(&route.id);
                Err(e)
            }
        }
    }

    async fn execute_steps_inner(
        &self,
        route: &mut RouteExtended,
        chains: &[crate::types::Chain],
        execute_in_background: bool,
    ) -> Result<()> {
        let state = self.execution_state();

        for step_idx in 0..route.steps.len() {
            if state.get(&route.id).is_none() {
                tracing::debug!(route_id = %route.id, "execution stopped externally");
                break;
            }

            let step = &route.steps[step_idx];

            if let Some(ref exec) = step.execution
                && exec.status == ExecutionStatus::Done
            {
                tracing::debug!(step_id = %step.id, "skipping completed step");
                continue;
            }

            if step_idx > 0 {
                let prev_to_amount = route.steps[step_idx - 1]
                    .execution
                    .as_ref()
                    .and_then(|e| e.to_amount.clone());
                if let Some(to_amount) = prev_to_amount {
                    route.steps[step_idx].step.action.from_amount = Some(to_amount.clone());
                    if let Some(ref mut included) = route.steps[step_idx].step.included_steps
                        && let Some(first) = included.first_mut()
                    {
                        first.action.from_amount = Some(to_amount);
                    }
                }
            }

            let from_chain_id = route.steps[step_idx].step.action.from_chain_id;

            let chain = chains
                .iter()
                .find(|c| c.id == from_chain_id)
                .ok_or_else(|| LiFiError::Provider {
                    code: LiFiErrorCode::ProviderUnavailable,
                    message: format!("No chain info found for chain ID {from_chain_id:?}"),
                })?;

            let chain_type = chain.chain_type;
            let provider = self
                .find_provider(|p| p.chain_type() == chain_type)
                .ok_or_else(|| LiFiError::Provider {
                    code: LiFiErrorCode::ProviderUnavailable,
                    message: format!("No provider registered for chain type {chain_type:?}"),
                })?;

            let route_id = route.base.id.clone();
            let mut executor =
                create_executor(provider.as_ref(), route_id.clone(), execute_in_background).await?;

            let step_ref = &mut route.steps[step_idx];

            tracing::info!(
                step_id = %step_ref.id,
                step_type = ?step_ref.step_type,
                "executing step"
            );

            match executor
                .execute_step(self, step_ref, provider.as_ref())
                .await
            {
                Ok(()) => {}
                Err(LiFiError::StepRetry { message, .. }) => {
                    tracing::info!(
                        step_id = %step_ref.id,
                        reason = %message,
                        "step retry requested, clearing execution and retrying"
                    );
                    step_ref.execution = None;
                    executor =
                        create_executor(provider.as_ref(), route_id.clone(), execute_in_background)
                            .await?;
                    executor
                        .execute_step(self, step_ref, provider.as_ref())
                        .await?;
                }
                Err(e) => return Err(e),
            }

            if step_ref
                .execution
                .as_ref()
                .is_none_or(|e| e.status != ExecutionStatus::Done)
            {
                tracing::info!(
                    step_id = %step_ref.id,
                    "step not done, stopping route execution"
                );
                self.stop_route_execution(&route_id);
            }

            state.with_route(&route_id, |data| {
                if step_idx < data.route.steps.len() {
                    data.route.steps[step_idx] = step_ref.clone();
                }
            });

            if !executor.allow_execution() {
                tracing::debug!("executor disallowed further execution, returning early");
                return Ok(());
            }
        }

        Ok(())
    }
}
