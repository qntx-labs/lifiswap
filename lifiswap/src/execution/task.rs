//! Execution task trait and pipeline.

use async_trait::async_trait;

use super::status::StatusManager;
use crate::LiFiClient;
use crate::error::Result;
use crate::provider::Provider;
use crate::types::{LiFiStepExtended, TaskStatus};

/// Context passed to each task in the execution pipeline.
pub struct ExecutionContext<'a> {
    /// The SDK client for API calls.
    pub client: &'a LiFiClient,
    /// The step being executed (mutable for status updates).
    pub step: &'a mut LiFiStepExtended,
    /// Status manager for tracking actions.
    pub status_manager: &'a StatusManager,
    /// Chain provider for on-chain queries (balance, allowance, etc.).
    pub provider: &'a dyn Provider,
    /// The route ID this step belongs to.
    pub route_id: &'a str,
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::execution::state::ExecutionState;
    use crate::execution::status::StatusManager;
    use crate::provider::Provider;
    use crate::types::{
        Action, ChainId, ChainType, LiFiStepExtended, StepExecutorOptions, Token, TokenAmount,
    };

    struct MockProvider;

    #[async_trait]
    impl Provider for MockProvider {
        fn chain_type(&self) -> ChainType {
            ChainType::EVM
        }
        fn is_address(&self, _address: &str) -> bool {
            true
        }
        async fn resolve_address(
            &self,
            _name: &str,
            _chain_id: Option<u64>,
        ) -> Result<Option<String>> {
            Ok(None)
        }
        async fn get_balance(&self, _wallet: &str, _tokens: &[Token]) -> Result<Vec<TokenAmount>> {
            Ok(vec![])
        }
        async fn create_step_executor(
            &self,
            _options: StepExecutorOptions,
        ) -> Result<Box<dyn crate::provider::StepExecutor>> {
            unimplemented!()
        }
    }

    static MOCK_PROVIDER: MockProvider = MockProvider;

    fn dummy_token() -> Token {
        Token {
            address: "0x0".to_owned(),
            decimals: 18,
            symbol: "TST".to_owned(),
            chain_id: ChainId(1),
            coin_key: None,
            name: "Test".to_owned(),
            logo_uri: None,
            price_usd: None,
        }
    }

    fn dummy_step() -> LiFiStepExtended {
        LiFiStepExtended {
            step: crate::types::LiFiStep {
                id: "s1".to_owned(),
                step_type: "swap".to_owned(),
                tool: None,
                tool_details: None,
                action: Action {
                    from_chain_id: ChainId(1),
                    to_chain_id: ChainId(1),
                    from_token: dummy_token(),
                    to_token: dummy_token(),
                    from_amount: None,
                    from_address: None,
                    to_address: None,
                    slippage: None,
                    destination_call_data: None,
                },
                estimate: None,
                included_steps: None,
                integrator: None,
                transaction_request: None,
                execution: None,
                typed_data: None,
                insurance: None,
            },
            execution: None,
        }
    }

    fn make_ctx<'a>(
        client: &'a LiFiClient,
        step: &'a mut LiFiStepExtended,
        mgr: &'a StatusManager,
    ) -> ExecutionContext<'a> {
        ExecutionContext {
            client,
            step,
            status_manager: mgr,
            provider: &MOCK_PROVIDER,
            route_id: "test-route",
            is_bridge_execution: false,
            allow_user_interaction: true,
        }
    }

    struct CompletingTask;
    #[async_trait]
    impl ExecutionTask for CompletingTask {
        async fn run(&self, _ctx: &mut ExecutionContext<'_>) -> Result<TaskStatus> {
            Ok(TaskStatus::Completed)
        }
    }

    struct PausingTask;
    #[async_trait]
    impl ExecutionTask for PausingTask {
        async fn run(&self, _ctx: &mut ExecutionContext<'_>) -> Result<TaskStatus> {
            Ok(TaskStatus::Paused)
        }
    }

    struct FailingTask;
    #[async_trait]
    impl ExecutionTask for FailingTask {
        async fn run(&self, _ctx: &mut ExecutionContext<'_>) -> Result<TaskStatus> {
            Err(crate::error::LiFiError::Validation("boom".to_owned()))
        }
    }

    struct CountingTask(Arc<AtomicU32>);
    #[async_trait]
    impl ExecutionTask for CountingTask {
        async fn run(&self, _ctx: &mut ExecutionContext<'_>) -> Result<TaskStatus> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(TaskStatus::Completed)
        }
    }

    struct SkippedTask;
    #[async_trait]
    impl ExecutionTask for SkippedTask {
        async fn should_run(&self, _ctx: &ExecutionContext<'_>) -> bool {
            false
        }
        #[allow(clippy::unimplemented)]
        async fn run(&self, _ctx: &mut ExecutionContext<'_>) -> Result<TaskStatus> {
            unreachable!("SkippedTask::run should never be called");
        }
    }

    #[tokio::test]
    async fn pipeline_completes_all_tasks() {
        let client = LiFiClient::new(
            crate::client::LiFiConfig::builder()
                .integrator("test")
                .build(),
        )
        .unwrap();
        let state = ExecutionState::new();
        let mgr = StatusManager::new("r1".to_owned(), state);
        let mut step = dummy_step();

        let counter = Arc::new(AtomicU32::new(0));
        let pipeline = TaskPipeline::new(vec![
            Box::new(CountingTask(Arc::clone(&counter))),
            Box::new(CountingTask(Arc::clone(&counter))),
            Box::new(CountingTask(Arc::clone(&counter))),
        ]);

        let result = pipeline.run(&mut make_ctx(&client, &mut step, &mgr)).await;
        assert_eq!(result.unwrap(), TaskStatus::Completed);
        assert_eq!(counter.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn pipeline_stops_on_pause() {
        let client = LiFiClient::new(
            crate::client::LiFiConfig::builder()
                .integrator("test")
                .build(),
        )
        .unwrap();
        let state = ExecutionState::new();
        let mgr = StatusManager::new("r1".to_owned(), state);
        let mut step = dummy_step();

        let counter = Arc::new(AtomicU32::new(0));
        let pipeline = TaskPipeline::new(vec![
            Box::new(CountingTask(Arc::clone(&counter))),
            Box::new(PausingTask),
            Box::new(CountingTask(Arc::clone(&counter))),
        ]);

        let result = pipeline.run(&mut make_ctx(&client, &mut step, &mgr)).await;
        assert_eq!(result.unwrap(), TaskStatus::Paused);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn pipeline_stops_on_error() {
        let client = LiFiClient::new(
            crate::client::LiFiConfig::builder()
                .integrator("test")
                .build(),
        )
        .unwrap();
        let state = ExecutionState::new();
        let mgr = StatusManager::new("r1".to_owned(), state);
        let mut step = dummy_step();

        let counter = Arc::new(AtomicU32::new(0));
        let pipeline = TaskPipeline::new(vec![
            Box::new(CountingTask(Arc::clone(&counter))),
            Box::new(FailingTask),
            Box::new(CountingTask(Arc::clone(&counter))),
        ]);

        let result = pipeline.run(&mut make_ctx(&client, &mut step, &mgr)).await;
        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn pipeline_skips_tasks_where_should_run_false() {
        let client = LiFiClient::new(
            crate::client::LiFiConfig::builder()
                .integrator("test")
                .build(),
        )
        .unwrap();
        let state = ExecutionState::new();
        let mgr = StatusManager::new("r1".to_owned(), state);
        let mut step = dummy_step();

        let pipeline = TaskPipeline::new(vec![
            Box::new(CompletingTask),
            Box::new(SkippedTask),
            Box::new(CompletingTask),
        ]);

        let result = pipeline.run(&mut make_ctx(&client, &mut step, &mgr)).await;
        assert_eq!(result.unwrap(), TaskStatus::Completed);
    }

    #[tokio::test]
    async fn empty_pipeline_completes() {
        let client = LiFiClient::new(
            crate::client::LiFiConfig::builder()
                .integrator("test")
                .build(),
        )
        .unwrap();
        let state = ExecutionState::new();
        let mgr = StatusManager::new("r1".to_owned(), state);
        let mut step = dummy_step();

        let pipeline = TaskPipeline::new(vec![]);
        let result = pipeline.run(&mut make_ctx(&client, &mut step, &mgr)).await;
        assert_eq!(result.unwrap(), TaskStatus::Completed);
    }
}
