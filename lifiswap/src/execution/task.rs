//! Execution task trait and pipeline.

use std::future::Future;
use std::pin::Pin;

use super::status::StatusManager;
use crate::LiFiClient;
use crate::error::Result;
use crate::provider::Provider;
use crate::types::{Chain, ExecutionOptions, LiFiStepExtended, SignedTypedData, TaskStatus};

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
    /// Execution options (hooks, background mode).
    pub execution_options: &'a ExecutionOptions,
    /// Whether this is a cross-chain bridge execution.
    pub is_bridge_execution: bool,
    /// Whether user interaction is allowed.
    pub allow_user_interaction: bool,
    /// Source chain metadata (for explorer URLs, etc.).
    pub from_chain: &'a Chain,
    /// Signed typed data accumulated during the pipeline (permits, etc.).
    pub signed_typed_data: Vec<SignedTypedData>,
}

impl std::fmt::Debug for ExecutionContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionContext")
            .field("step_id", &self.step.id)
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
pub trait ExecutionTask: Send + Sync {
    /// Whether this task should run given the current context.
    ///
    /// Default: always runs.
    fn should_run<'a>(
        &'a self,
        _ctx: &'a ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async { true })
    }

    /// Execute the task, returning whether the pipeline should continue or pause.
    ///
    /// # Errors
    ///
    /// Returns an error if the task fails (e.g. insufficient balance, tx failure).
    fn run<'a>(
        &'a self,
        ctx: &'a mut ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = Result<TaskStatus>> + Send + 'a>>;
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
    use crate::execution::test_helpers::dummy_chain;
    use crate::provider::Provider;
    use crate::types::{ChainType, LiFiStepExtended, StepExecutorOptions, Token, TokenAmount};

    struct MockProvider;

    impl Provider for MockProvider {
        fn chain_type(&self) -> ChainType {
            ChainType::EVM
        }
        fn is_address(&self, _address: &str) -> bool {
            true
        }
        fn resolve_address<'a>(
            &'a self,
            _name: &'a str,
            _chain_id: Option<u64>,
        ) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + 'a>> {
            Box::pin(async { Ok(None) })
        }
        fn get_balance<'a>(
            &'a self,
            _wallet: &'a str,
            _tokens: &'a [Token],
        ) -> Pin<Box<dyn Future<Output = Result<Vec<TokenAmount>>> + Send + 'a>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn create_step_executor<'a>(
            &'a self,
            _options: StepExecutorOptions,
        ) -> Pin<Box<dyn Future<Output = Result<Box<dyn crate::provider::StepExecutor>>> + Send + 'a>>
        {
            Box::pin(async {
                Err(crate::error::LiFiError::Config(
                    "MockProvider does not support step execution".to_owned(),
                ))
            })
        }
    }

    static MOCK_PROVIDER: MockProvider = MockProvider;

    use crate::execution::test_helpers::dummy_step;

    static DEFAULT_OPTS: ExecutionOptions = ExecutionOptions {
        update_route_hook: None,
        accept_exchange_rate_update_hook: None,
        update_transaction_request_hook: None,
        execute_in_background: false,
    };

    static MOCK_CHAIN: std::sync::LazyLock<Chain> = std::sync::LazyLock::new(dummy_chain);

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
            execution_options: &DEFAULT_OPTS,
            is_bridge_execution: false,
            allow_user_interaction: true,
            from_chain: &MOCK_CHAIN,
            signed_typed_data: Vec::new(),
        }
    }

    struct CompletingTask;
    impl ExecutionTask for CompletingTask {
        fn run<'a>(
            &'a self,
            _ctx: &'a mut ExecutionContext<'_>,
        ) -> Pin<Box<dyn Future<Output = Result<TaskStatus>> + Send + 'a>> {
            Box::pin(async { Ok(TaskStatus::Completed) })
        }
    }

    struct PausingTask;
    impl ExecutionTask for PausingTask {
        fn run<'a>(
            &'a self,
            _ctx: &'a mut ExecutionContext<'_>,
        ) -> Pin<Box<dyn Future<Output = Result<TaskStatus>> + Send + 'a>> {
            Box::pin(async { Ok(TaskStatus::Paused) })
        }
    }

    struct FailingTask;
    impl ExecutionTask for FailingTask {
        fn run<'a>(
            &'a self,
            _ctx: &'a mut ExecutionContext<'_>,
        ) -> Pin<Box<dyn Future<Output = Result<TaskStatus>> + Send + 'a>> {
            Box::pin(async { Err(crate::error::LiFiError::Validation("boom".to_owned())) })
        }
    }

    struct CountingTask(Arc<AtomicU32>);
    impl ExecutionTask for CountingTask {
        fn run<'a>(
            &'a self,
            _ctx: &'a mut ExecutionContext<'_>,
        ) -> Pin<Box<dyn Future<Output = Result<TaskStatus>> + Send + 'a>> {
            Box::pin(async move {
                self.0.fetch_add(1, Ordering::Relaxed);
                Ok(TaskStatus::Completed)
            })
        }
    }

    struct SkippedTask;
    impl ExecutionTask for SkippedTask {
        fn should_run<'a>(
            &'a self,
            _ctx: &'a ExecutionContext<'_>,
        ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
            Box::pin(async { false })
        }
        #[allow(clippy::unimplemented)]
        fn run<'a>(
            &'a self,
            _ctx: &'a mut ExecutionContext<'_>,
        ) -> Pin<Box<dyn Future<Output = Result<TaskStatus>> + Send + 'a>> {
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
        let mut step = dummy_step("s1");

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
        let mut step = dummy_step("s1");

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
        let mut step = dummy_step("s1");

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
        let mut step = dummy_step("s1");

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
        let mut step = dummy_step("s1");

        let pipeline = TaskPipeline::new(vec![]);
        let result = pipeline.run(&mut make_ctx(&client, &mut step, &mgr)).await;
        assert_eq!(result.unwrap(), TaskStatus::Completed);
    }
}
