//! Execution task trait and pipeline.

use async_trait::async_trait;

use super::status::StatusManager;
use crate::LiFiClient;
use crate::error::Result;
use crate::types::{LiFiStepExtended, TaskStatus};

/// Context passed to each task in the execution pipeline.
pub struct ExecutionContext<'a> {
    /// The SDK client for API calls.
    pub client: &'a LiFiClient,
    /// The step being executed (mutable for status updates).
    pub step: &'a mut LiFiStepExtended,
    /// Status manager for tracking actions.
    pub status_manager: &'a StatusManager,
    /// Whether this is a cross-chain bridge execution.
    pub is_bridge_execution: bool,
    /// Whether user interaction is allowed.
    pub allow_user_interaction: bool,
}

impl std::fmt::Debug for ExecutionContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionContext")
            .field("step_id", &self.step.step.id)
            .field("is_bridge_execution", &self.is_bridge_execution)
            .field("allow_user_interaction", &self.allow_user_interaction)
            .finish()
    }
}

/// A single task within the step execution pipeline.
///
/// Chain-specific crates define concrete tasks (e.g. `CheckAllowanceTask`,
/// `SignAndExecuteTask`). The core crate provides generic tasks like
/// [`CheckBalanceTask`](super::tasks::CheckBalanceTask) and
/// [`PrepareTransactionTask`](super::tasks::PrepareTransactionTask).
#[async_trait]
pub trait ExecutionTask: Send + Sync {
    /// Whether this task should run given the current context.
    ///
    /// Default: always runs.
    async fn should_run(&self, _ctx: &ExecutionContext<'_>) -> bool {
        true
    }

    /// Execute the task, returning whether the pipeline should continue or pause.
    ///
    /// # Errors
    ///
    /// Returns an error if the task fails (e.g. insufficient balance, tx failure).
    async fn run(&self, ctx: &mut ExecutionContext<'_>) -> Result<TaskStatus>;
}

/// A sequential pipeline of execution tasks.
///
/// Tasks are run in order. Each task can:
/// - **Complete** → pipeline proceeds to the next task
/// - **Pause** → pipeline stops, can be resumed later
/// - **Error** → pipeline stops with an error
pub struct TaskPipeline {
    tasks: Vec<Box<dyn ExecutionTask>>,
}

impl std::fmt::Debug for TaskPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskPipeline")
            .field("tasks_count", &self.tasks.len())
            .finish()
    }
}

impl TaskPipeline {
    /// Create a new pipeline from a list of tasks.
    #[must_use]
    pub fn new(tasks: Vec<Box<dyn ExecutionTask>>) -> Self {
        Self { tasks }
    }

    /// Run all tasks in sequence.
    ///
    /// # Errors
    ///
    /// Returns the first error encountered during task execution.
    pub async fn run(&self, ctx: &mut ExecutionContext<'_>) -> Result<TaskStatus> {
        for task in &self.tasks {
            if !task.should_run(ctx).await {
                continue;
            }
            let status = task.run(ctx).await?;
            if status == TaskStatus::Paused {
                return Ok(TaskStatus::Paused);
            }
        }
        Ok(TaskStatus::Completed)
    }
}
