//! Execution state for tracking active route executions.
//!
//! [`ExecutionState`] is cheaply cloneable (`Arc`-backed) and can be
//! shared across tasks without lifetime concerns. It lives inside
//! [`LiFiClient`](crate::LiFiClient) and is automatically available
//! during route execution.

use std::sync::Arc;

use dashmap::DashMap;

use crate::provider::StepExecutor;
use crate::types::{ExecutionOptions, RouteExtended};

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
/// Cheaply cloneable — all clones share the same underlying map.
/// Uses [`DashMap`] for lock-free concurrent access.
#[derive(Debug, Clone)]
pub struct ExecutionState {
    state: Arc<DashMap<String, ExecutionData>>,
}

impl ExecutionState {
    /// Create a new empty execution state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(DashMap::new()),
        }
    }

    /// Get a reference to execution data for a route.
    #[must_use]
    pub fn get(
        &self,
        route_id: &str,
    ) -> Option<dashmap::mapref::one::Ref<'_, String, ExecutionData>> {
        self.state.get(route_id)
    }

    /// Create or update execution data for a route.
    pub fn create(&self, route: RouteExtended, execution_options: ExecutionOptions) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::test_helpers::dummy_route;
    use crate::types::ExecutionOptions;

    #[test]
    fn create_and_get() {
        let state = ExecutionState::new();
        let route = dummy_route("r1");
        state.create(route, ExecutionOptions::default());

        assert!(state.get("r1").is_some());
        assert!(state.get("r2").is_none());
    }

    #[test]
    fn delete_removes_entry() {
        let state = ExecutionState::new();
        state.create(dummy_route("r1"), ExecutionOptions::default());

        state.delete("r1");
        assert!(state.get("r1").is_none());
    }

    #[test]
    fn active_route_ids() {
        let state = ExecutionState::new();
        state.create(dummy_route("a"), ExecutionOptions::default());
        state.create(dummy_route("b"), ExecutionOptions::default());

        let mut ids = state.active_route_ids();
        ids.sort();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn update_replaces_route_data() {
        let state = ExecutionState::new();
        state.create(dummy_route("r1"), ExecutionOptions::default());

        let mut updated = dummy_route("r1");
        updated.from_amount = "2000".to_owned();
        state.update(updated, ExecutionOptions::default());

        let data = state.get("r1").unwrap();
        assert_eq!(data.route.from_amount, "2000");
    }

    #[test]
    fn with_route_mutates() {
        let state = ExecutionState::new();
        state.create(dummy_route("r1"), ExecutionOptions::default());

        state.with_route("r1", |data| {
            data.route.to_amount = "500".to_owned();
        });

        let data = state.get("r1").unwrap();
        assert_eq!(data.route.to_amount, "500");
    }

    #[test]
    fn with_route_noop_on_missing() {
        let state = ExecutionState::new();
        state.with_route("missing", |_| unreachable!("closure should not be called"));
    }

    #[test]
    fn clone_shares_state() {
        let state = ExecutionState::new();
        let clone = state.clone();

        state.create(dummy_route("r1"), ExecutionOptions::default());
        assert!(clone.get("r1").is_some());
    }

    #[test]
    fn create_preserves_existing_executors() {
        let state = ExecutionState::new();
        state.create(dummy_route("r1"), ExecutionOptions::default());

        // Re-create with same ID should not panic
        state.create(dummy_route("r1"), ExecutionOptions::default());
        assert!(state.get("r1").is_some());
    }
}
