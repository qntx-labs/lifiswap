//! Status manager for tracking step execution progress.

use std::time::{SystemTime, UNIX_EPOCH};

use super::state::EXECUTION_STATE;
use crate::types::{
    ExecutionAction, ExecutionActionStatus, ExecutionActionType, ExecutionError, ExecutionStatus,
    LiFiStepExtended, StepExecution,
};

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Manages execution status updates for a single route.
///
/// Tracks actions within each step and propagates updates to the
/// global [`ExecutionState`](super::state::ExecutionState) and
/// optional update hooks.
#[derive(Debug)]
pub struct StatusManager {
    route_id: String,
    should_update: bool,
}

impl StatusManager {
    /// Create a new status manager for the given route.
    #[must_use]
    pub const fn new(route_id: String) -> Self {
        Self {
            route_id,
            should_update: true,
        }
    }

    /// Initialize the execution state of a step.
    ///
    /// If the step has no execution state, creates a new one with `Pending` status.
    /// If the step was previously `Failed`, resets it to `Pending` for retry.
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

        step.execution.clone().expect("execution was just set")
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
    /// # Panics
    ///
    /// Panics if execution has not been initialized.
    pub fn create_action(
        &self,
        step: &mut LiFiStepExtended,
        action_type: ExecutionActionType,
        chain_id: u64,
        status: ExecutionActionStatus,
    ) -> ExecutionAction {
        let exec = step
            .execution
            .as_mut()
            .expect("execution must be initialized before creating actions");

        let action = ExecutionAction {
            action_type,
            status,
            message: None,
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
        action
    }

    /// Find or create an action of the given type.
    pub fn initialize_action(
        &self,
        step: &mut LiFiStepExtended,
        action_type: ExecutionActionType,
        chain_id: u64,
        status: ExecutionActionStatus,
    ) -> ExecutionAction {
        if self.find_action(step, action_type).is_some() {
            self.update_action(step, action_type, status, None);
            return self
                .find_action(step, action_type)
                .cloned()
                .expect("action was just found");
        }
        self.create_action(step, action_type, chain_id, status)
    }

    /// Update an existing action's status and optional extra fields.
    pub fn update_action(
        &self,
        step: &mut LiFiStepExtended,
        action_type: ExecutionActionType,
        status: ExecutionActionStatus,
        params: Option<ActionUpdateParams>,
    ) {
        let exec = step
            .execution
            .as_mut()
            .expect("execution must be initialized");

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
            }
        }

        // Sort: DONE actions first
        exec.actions
            .sort_by_key(|a| i32::from(a.status != ExecutionActionStatus::Done));

        self.update_step_in_route(step);
    }

    /// Enable or disable status update propagation.
    pub const fn allow_updates(&mut self, value: bool) {
        self.should_update = value;
    }

    fn update_step_in_route(&self, step: &LiFiStepExtended) {
        if !self.should_update {
            return;
        }

        EXECUTION_STATE.with_route(&self.route_id, |data| {
            if let Some(step_idx) = data
                .route
                .steps
                .iter()
                .position(|s| s.step.id == step.step.id)
            {
                data.route.steps[step_idx] = step.clone();
            }

            if let Some(ref hook) = data.execution_options.update_route_hook {
                hook(&data.route);
            }
        });
    }
}

/// Partial update for a step's execution state.
#[derive(Debug, Default)]
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
#[derive(Debug, Default, Clone)]
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
}
