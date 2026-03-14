//! Global execution state for tracking active route executions.

use dashmap::DashMap;
use std::sync::LazyLock;

use crate::provider::StepExecutor;
use crate::types::{ExecutionOptions, RouteExtended};

/// Global execution state singleton.
pub static EXECUTION_STATE: LazyLock<ExecutionState> = LazyLock::new(ExecutionState::new);

/// Data associated with an active route execution.
pub struct ExecutionData {
    /// The route being executed (with execution state).
    pub route: RouteExtended,
    /// Step executors created by providers.
    pub executors: Vec<Box<dyn StepExecutor>>,
    /// Execution options (hooks, background mode).
    pub execution_options: ExecutionOptions,
}

impl std::fmt::Debug for ExecutionData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionData")
            .field("route_id", &self.route.id)
            .field("executors_count", &self.executors.len())
            .field("execution_options", &self.execution_options)
            .finish()
    }
}

/// Thread-safe storage for active route executions.
///
/// Uses [`DashMap`] for concurrent access from multiple tasks.
#[derive(Debug)]
pub struct ExecutionState {
    state: DashMap<String, ExecutionData>,
}

impl ExecutionState {
    /// Create a new empty execution state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: DashMap::new(),
        }
    }

    /// Get a reference to execution data for a route.
    #[must_use]
    pub fn get(&self, route_id: &str) -> Option<dashmap::mapref::one::Ref<'_, String, ExecutionData>> {
        self.state.get(route_id)
    }

    /// Create or update execution data for a route.
    pub fn create(
        &self,
        route: RouteExtended,
        execution_options: ExecutionOptions,
    ) -> String {
        let route_id = route.id.clone();
        let existing_executors = self
            .state
            .remove(&route_id)
            .map_or_else(Vec::new, |(_, old)| old.executors);

        self.state.insert(
            route_id.clone(),
            ExecutionData {
                route,
                executors: existing_executors,
                execution_options,
            },
        );
        route_id
    }

    /// Update the route and options for an existing execution.
    pub fn update(&self, route: RouteExtended, execution_options: ExecutionOptions) {
        let route_id = route.id.clone();
        self.state.alter(&route_id, |_key, mut data| {
            data.route = route;
            data.execution_options = execution_options;
            data
        });
    }

    /// Remove execution data for a route.
    pub fn delete(&self, route_id: &str) {
        self.state.remove(route_id);
    }

    /// Get all active route IDs.
    #[must_use]
    pub fn active_route_ids(&self) -> Vec<String> {
        self.state.iter().map(|e| e.key().clone()).collect()
    }

    /// Execute a closure with mutable access to an execution data entry.
    ///
    /// Does nothing if the route ID is not found.
    pub fn with_route(&self, route_id: &str, f: impl FnOnce(&mut ExecutionData)) {
        if let Some(mut entry) = self.state.get_mut(route_id) {
            f(entry.value_mut());
        }
    }
}

impl Default for ExecutionState {
    fn default() -> Self {
        Self::new()
    }
}
