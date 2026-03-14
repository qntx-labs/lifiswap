//! Prepare a route for restart after a failed or paused execution.
//!
//! Mirrors the `TypeScript` SDK's `prepareRestart.ts`: resets execution
//! actions on each step so the route can be re-executed cleanly.

use crate::types::{ExecutionActionStatus, ExecutionActionType, RouteExtended};

/// Valid action types that indicate a committed on-chain transaction.
const TRANSACTION_ACTION_TYPES: [ExecutionActionType; 3] = [
    ExecutionActionType::Swap,
    ExecutionActionType::CrossChain,
    ExecutionActionType::ReceivingChain,
];

/// Prepare a route for re-execution by cleaning up stale execution state.
///
/// For each step:
/// 1. If the step has execution actions, find the last action that has a
///    `tx_hash` or `task_id` and is not `FAILED`.
/// 2. Truncate actions after that point (keep the committed ones).
/// 3. If no such action exists, clear all actions.
/// 4. Reset `last_action_type`.
/// 5. Clear `transaction_request`.
pub fn prepare_restart(route: &mut RouteExtended) {
    for step in &mut route.steps {
        if let Some(ref mut execution) = step.execution {
            let last_valid_idx = execution.actions.iter().rposition(|action| {
                TRANSACTION_ACTION_TYPES.contains(&action.action_type)
                    && (action.tx_hash.is_some() || action.task_id.is_some())
                    && action.status != ExecutionActionStatus::Failed
            });

            if let Some(idx) = last_valid_idx {
                execution.actions.truncate(idx + 1);
            } else {
                execution.actions.clear();
            }

            execution.last_action_type = None;
        }

        step.step.transaction_request = None;
    }
}
