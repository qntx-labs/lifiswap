//! Status manager for tracking step execution progress.

use std::time::{SystemTime, UNIX_EPOCH};

use super::messages::get_action_message;
use super::state::ExecutionState;
use crate::error::{LiFiError, Result};
use crate::types::{
    ExecutionAction, ExecutionActionStatus, ExecutionActionType, ExecutionError, ExecutionStatus,
    LiFiStepExtended, StepExecution, TransactionMethodType,
};

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn execution_not_initialized() -> LiFiError {
    LiFiError::Execution("Execution has not been initialized on this step.".to_owned())
}

/// Manages execution status updates for a single route.
///
/// Tracks actions within each step and propagates updates to the
/// client's [`ExecutionState`](super::state::ExecutionState) and
/// optional update hooks.
#[derive(Debug, Clone)]
pub struct StatusManager {
    route_id: String,
    state: ExecutionState,
    should_update: bool,
}

impl StatusManager {
    /// Create a new status manager for the given route.
    #[must_use]
    pub const fn new(route_id: String, state: ExecutionState) -> Self {
        Self {
            route_id,
            state,
            should_update: true,
        }
    }

    /// Initialize the execution state of a step.
    ///
    /// If the step has no execution state, creates a new one with `Pending` status.
    /// If the step was previously `Failed`, resets it to `Pending` for retry.
    ///
    /// # Panics
    ///
    /// Panics if the execution field is `None` after initialization (should never happen).
    pub fn initialize_execution(&self, step: &mut LiFiStepExtended) -> StepExecution {
        if step.execution.is_none() {
            step.execution = Some(StepExecution {
                started_at: now_ms(),
                signed_at: None,
                status: ExecutionStatus::Pending,
                actions: Vec::new(),
                last_action_type: None,
                from_amount: None,
                to_amount: None,
                to_token: None,
                fee_costs: None,
                gas_costs: None,
                internal_tx_link: None,
                external_tx_link: None,
                error: None,
            });
            self.update_step_in_route(step);
        }

        if let Some(ref mut exec) = step.execution
            && exec.status == ExecutionStatus::Failed
        {
            exec.started_at = now_ms();
            exec.status = ExecutionStatus::Pending;
            exec.signed_at = None;
            exec.last_action_type = None;
            exec.error = None;
            self.update_step_in_route(step);
        }

        step.execution
            .clone()
            .expect("execution was just initialized above")
    }

    /// Update the execution state of a step with partial data.
    pub fn update_execution(&self, step: &mut LiFiStepExtended, update: ExecutionUpdate) {
        if let Some(ref mut exec) = step.execution {
            if let Some(status) = update.status {
                exec.status = status;
            }
            if let Some(from_amount) = update.from_amount {
                exec.from_amount = Some(from_amount);
            }
            if let Some(to_amount) = update.to_amount {
                exec.to_amount = Some(to_amount);
            }
            if let Some(to_token) = update.to_token {
                exec.to_token = Some(to_token);
            }
            if let Some(gas_costs) = update.gas_costs {
                exec.gas_costs = Some(gas_costs);
            }
            if let Some(internal_tx_link) = update.internal_tx_link {
                exec.internal_tx_link = Some(internal_tx_link);
            }
            if let Some(external_tx_link) = update.external_tx_link {
                exec.external_tx_link = Some(external_tx_link);
            }
            if update.error.is_some() {
                exec.error = update.error;
            }
            self.update_step_in_route(step);
        }
    }

    /// Find an action of the given type in the step's execution.
    #[must_use]
    pub fn find_action<'a>(
        &self,
        step: &'a LiFiStepExtended,
        action_type: ExecutionActionType,
    ) -> Option<&'a ExecutionAction> {
        step.execution
            .as_ref()?
            .actions
            .iter()
            .find(move |a| a.action_type == action_type)
    }

    /// Create and push a new action into the step's execution.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError::Execution`] if execution has not been initialized.
    pub fn create_action(
        &self,
        step: &mut LiFiStepExtended,
        action_type: ExecutionActionType,
        chain_id: u64,
        status: ExecutionActionStatus,
    ) -> Result<ExecutionAction> {
        let exec = step
            .execution
            .as_mut()
            .ok_or_else(execution_not_initialized)?;

        let action = ExecutionAction {
            action_type,
            status,
            message: get_action_message(action_type, status).map(String::from),
            chain_id: Some(chain_id),
            tx_hash: None,
            tx_link: None,
            task_id: None,
            tx_type: None,
            error: None,
            substatus: None,
            substatus_message: None,
        };

        exec.actions.push(action.clone());
        exec.last_action_type = Some(action_type);
        self.update_step_in_route(step);
        Ok(action)
    }

    /// Find or create an action of the given type.
    ///
    /// If an action with the given type already exists, updates its status.
    /// Otherwise creates a new action.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError::Execution`] if execution has not been initialized.
    pub fn initialize_action(
        &self,
        step: &mut LiFiStepExtended,
        action_type: ExecutionActionType,
        chain_id: u64,
        status: ExecutionActionStatus,
    ) -> Result<ExecutionAction> {
        if self.find_action(step, action_type).is_some() {
            self.update_action(step, action_type, status, None)?;
            return self.find_action(step, action_type).cloned().ok_or_else(|| {
                LiFiError::Execution(format!("Action {action_type:?} not found after update."))
            });
        }
        self.create_action(step, action_type, chain_id, status)
    }

    /// Update an existing action's status and optional extra fields.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError::Execution`] if execution has not been initialized.
    pub fn update_action(
        &self,
        step: &mut LiFiStepExtended,
        action_type: ExecutionActionType,
        status: ExecutionActionStatus,
        params: Option<ActionUpdateParams>,
    ) -> Result<()> {
        let exec = step
            .execution
            .as_mut()
            .ok_or_else(execution_not_initialized)?;

        match status {
            ExecutionActionStatus::Failed => {
                exec.status = ExecutionStatus::Failed;
                if let Some(ref p) = params
                    && p.error.is_some()
                {
                    exec.error.clone_from(&p.error);
                }
            }
            ExecutionActionStatus::Pending => {
                exec.status = ExecutionStatus::Pending;
                if let Some(ref p) = params
                    && let Some(signed_at) = p.signed_at
                {
                    exec.signed_at = Some(signed_at);
                }
            }
            ExecutionActionStatus::ActionRequired
            | ExecutionActionStatus::MessageRequired
            | ExecutionActionStatus::ResetRequired => {
                exec.status = ExecutionStatus::ActionRequired;
            }
            _ => {}
        }

        exec.last_action_type = Some(action_type);

        if let Some(action) = exec
            .actions
            .iter_mut()
            .find(|a| a.action_type == action_type)
        {
            action.status = status;
            action.message = get_action_message(action_type, status)
                .map(String::from)
                .or_else(|| action.message.take());
            if let Some(p) = params {
                if let Some(chain_id) = p.chain_id {
                    action.chain_id = Some(chain_id);
                }
                if let Some(tx_hash) = p.tx_hash {
                    action.tx_hash = Some(tx_hash);
                }
                if let Some(tx_link) = p.tx_link {
                    action.tx_link = Some(tx_link);
                }
                if p.error.is_some() {
                    action.error.clone_from(&p.error);
                }
                if let Some(substatus) = p.substatus {
                    action.substatus = Some(substatus);
                }
                if let Some(msg) = p.substatus_message {
                    action.substatus_message = Some(msg);
                }
                if let Some(task_id) = p.task_id {
                    action.task_id = Some(task_id);
                }
                if let Some(tx_type) = p.tx_type {
                    action.tx_type = Some(tx_type);
                }
            }
        }

        // Sort: DONE actions first
        exec.actions
            .sort_by_key(|a| i32::from(a.status != ExecutionActionStatus::Done));

        self.update_step_in_route(step);
        Ok(())
    }

    /// Enable or disable status update propagation.
    pub const fn allow_updates(&mut self, value: bool) {
        self.should_update = value;
    }

    fn update_step_in_route(&self, step: &LiFiStepExtended) {
        if !self.should_update {
            return;
        }

        self.state.with_route(&self.route_id, |data| {
            if let Some(step_idx) = data.route.steps.iter().position(|s| s.id == step.id) {
                data.route.steps[step_idx] = step.clone();
            }

            if let Some(ref hook) = data.execution_options.update_route_hook {
                hook(&data.route);
            }
        });
    }
}

/// Partial update for a step's execution state.
#[derive(Debug, Default, bon::Builder)]
pub struct ExecutionUpdate {
    /// New status.
    pub status: Option<ExecutionStatus>,
    /// Actual source amount.
    pub from_amount: Option<String>,
    /// Actual destination amount.
    pub to_amount: Option<String>,
    /// Actual destination token.
    pub to_token: Option<crate::types::Token>,
    /// Gas costs.
    pub gas_costs: Option<Vec<crate::types::GasCost>>,
    /// Internal explorer link.
    pub internal_tx_link: Option<String>,
    /// External explorer link.
    pub external_tx_link: Option<String>,
    /// Error details.
    pub error: Option<ExecutionError>,
}

/// Optional parameters for updating an action.
#[derive(Debug, Default, Clone, bon::Builder)]
pub struct ActionUpdateParams {
    /// Chain ID override.
    pub chain_id: Option<u64>,
    /// Transaction hash.
    pub tx_hash: Option<String>,
    /// Transaction link.
    pub tx_link: Option<String>,
    /// Signed-at timestamp.
    pub signed_at: Option<u64>,
    /// Error details.
    pub error: Option<ExecutionError>,
    /// Substatus code.
    pub substatus: Option<String>,
    /// Substatus message.
    pub substatus_message: Option<String>,
    /// Task ID (for relay/batched transactions).
    pub task_id: Option<String>,
    /// Transaction method type (standard, relayed, batched).
    pub tx_type: Option<TransactionMethodType>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::test_helpers::dummy_step;

    fn make_manager() -> StatusManager {
        let state = ExecutionState::new();
        StatusManager::new("route-1".to_owned(), state)
    }

    #[test]
    fn initialize_execution_creates_pending() {
        let mgr = make_manager();
        let mut step = dummy_step("s1");

        let exec = mgr.initialize_execution(&mut step);
        assert_eq!(exec.status, ExecutionStatus::Pending);
        assert!(step.execution.is_some());
    }

    #[test]
    fn initialize_execution_resets_failed() {
        let mgr = make_manager();
        let mut step = dummy_step("s1");

        mgr.initialize_execution(&mut step);
        step.execution.as_mut().unwrap().status = ExecutionStatus::Failed;

        let exec = mgr.initialize_execution(&mut step);
        assert_eq!(exec.status, ExecutionStatus::Pending);
        assert!(exec.error.is_none());
    }

    #[test]
    fn create_action_returns_error_without_init() {
        let mgr = make_manager();
        let mut step = dummy_step("s1");

        let result = mgr.create_action(
            &mut step,
            ExecutionActionType::Swap,
            1,
            ExecutionActionStatus::Started,
        );
        assert!(result.is_err());
    }

    #[test]
    fn create_action_succeeds_after_init() {
        let mgr = make_manager();
        let mut step = dummy_step("s1");
        mgr.initialize_execution(&mut step);

        let action = mgr
            .create_action(
                &mut step,
                ExecutionActionType::Swap,
                1,
                ExecutionActionStatus::Started,
            )
            .unwrap();

        assert_eq!(action.action_type, ExecutionActionType::Swap);
        assert_eq!(action.status, ExecutionActionStatus::Started);
        assert_eq!(action.chain_id, Some(1));
    }

    #[test]
    fn find_action_returns_none_when_empty() {
        let mgr = make_manager();
        let mut step = dummy_step("s1");
        mgr.initialize_execution(&mut step);

        assert!(mgr.find_action(&step, ExecutionActionType::Swap).is_none());
    }

    #[test]
    fn find_action_returns_created_action() {
        let mgr = make_manager();
        let mut step = dummy_step("s1");
        mgr.initialize_execution(&mut step);
        mgr.create_action(
            &mut step,
            ExecutionActionType::Swap,
            1,
            ExecutionActionStatus::Started,
        )
        .unwrap();

        let found = mgr.find_action(&step, ExecutionActionType::Swap);
        assert!(found.is_some());
        assert_eq!(found.unwrap().action_type, ExecutionActionType::Swap);
    }

    #[test]
    fn initialize_action_creates_new() {
        let mgr = make_manager();
        let mut step = dummy_step("s1");
        mgr.initialize_execution(&mut step);

        let action = mgr
            .initialize_action(
                &mut step,
                ExecutionActionType::CrossChain,
                137,
                ExecutionActionStatus::Pending,
            )
            .unwrap();
        assert_eq!(action.action_type, ExecutionActionType::CrossChain);
    }

    #[test]
    fn initialize_action_updates_existing() {
        let mgr = make_manager();
        let mut step = dummy_step("s1");
        mgr.initialize_execution(&mut step);

        mgr.create_action(
            &mut step,
            ExecutionActionType::Swap,
            1,
            ExecutionActionStatus::Started,
        )
        .unwrap();

        let action = mgr
            .initialize_action(
                &mut step,
                ExecutionActionType::Swap,
                1,
                ExecutionActionStatus::Done,
            )
            .unwrap();
        assert_eq!(action.status, ExecutionActionStatus::Done);
    }

    #[test]
    fn update_action_sets_failed_status() {
        let mgr = make_manager();
        let mut step = dummy_step("s1");
        mgr.initialize_execution(&mut step);
        mgr.create_action(
            &mut step,
            ExecutionActionType::Swap,
            1,
            ExecutionActionStatus::Started,
        )
        .unwrap();

        mgr.update_action(
            &mut step,
            ExecutionActionType::Swap,
            ExecutionActionStatus::Failed,
            None,
        )
        .unwrap();

        assert_eq!(
            step.execution.as_ref().unwrap().status,
            ExecutionStatus::Failed
        );
    }

    #[test]
    fn update_action_with_tx_hash() {
        let mgr = make_manager();
        let mut step = dummy_step("s1");
        mgr.initialize_execution(&mut step);
        mgr.create_action(
            &mut step,
            ExecutionActionType::Swap,
            1,
            ExecutionActionStatus::Started,
        )
        .unwrap();

        mgr.update_action(
            &mut step,
            ExecutionActionType::Swap,
            ExecutionActionStatus::Pending,
            Some(ActionUpdateParams {
                tx_hash: Some("0xabc".to_owned()),
                signed_at: Some(12345),
                ..Default::default()
            }),
        )
        .unwrap();

        let action = mgr.find_action(&step, ExecutionActionType::Swap).unwrap();
        assert_eq!(action.tx_hash.as_deref(), Some("0xabc"));
        assert_eq!(step.execution.as_ref().unwrap().signed_at, Some(12345));
    }

    #[test]
    fn update_action_returns_error_without_init() {
        let mgr = make_manager();
        let mut step = dummy_step("s1");

        let result = mgr.update_action(
            &mut step,
            ExecutionActionType::Swap,
            ExecutionActionStatus::Done,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn update_execution_sets_fields() {
        let mgr = make_manager();
        let mut step = dummy_step("s1");
        mgr.initialize_execution(&mut step);

        mgr.update_execution(
            &mut step,
            ExecutionUpdate {
                status: Some(ExecutionStatus::Done),
                from_amount: Some("100".to_owned()),
                to_amount: Some("99".to_owned()),
                ..Default::default()
            },
        );

        let exec = step.execution.as_ref().unwrap();
        assert_eq!(exec.status, ExecutionStatus::Done);
        assert_eq!(exec.from_amount.as_deref(), Some("100"));
        assert_eq!(exec.to_amount.as_deref(), Some("99"));
    }

    #[test]
    fn allow_updates_disables_propagation() {
        let mgr = make_manager();
        let mut step = dummy_step("s1");
        // Just verify it doesn't panic when updates are disabled
        let mut mgr = mgr;
        mgr.allow_updates(false);
        mgr.initialize_execution(&mut step);
        assert!(step.execution.is_some());
    }

    #[test]
    fn done_actions_sorted_first() {
        let mgr = make_manager();
        let mut step = dummy_step("s1");
        mgr.initialize_execution(&mut step);

        mgr.create_action(
            &mut step,
            ExecutionActionType::CheckAllowance,
            1,
            ExecutionActionStatus::Started,
        )
        .unwrap();
        mgr.create_action(
            &mut step,
            ExecutionActionType::Swap,
            1,
            ExecutionActionStatus::Started,
        )
        .unwrap();

        mgr.update_action(
            &mut step,
            ExecutionActionType::Swap,
            ExecutionActionStatus::Done,
            None,
        )
        .unwrap();

        let actions = &step.execution.as_ref().unwrap().actions;
        assert_eq!(actions[0].status, ExecutionActionStatus::Done);
        assert_eq!(actions[0].action_type, ExecutionActionType::Swap);
    }
}
