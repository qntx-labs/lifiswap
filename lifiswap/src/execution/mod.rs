//! Route execution engine.
//!
//! This module provides the core execution pipeline for running cross-chain
//! routes returned by the LI.FI API. It coordinates [`Provider`](crate::provider::Provider)
//! implementations to execute each step in a route.
//!
//! # Architecture
//!
//! ```text
//! execute_route(route)
//!   └─ for each step:
//!        Provider::create_step_executor()
//!          └─ TaskPipeline::run()
//!               ├─ CheckBalanceTask
//!               ├─ PrepareTransactionTask
//!               ├─ (chain-specific tasks: allowance, sign, broadcast)
//!               └─ WaitForTransactionStatusTask
//! ```

pub mod engine;
pub mod state;
pub mod status;
pub mod task;
pub mod tasks;

pub use engine::{execute_route, get_active_route, get_active_routes, resume_route, stop_route_execution};
pub use state::ExecutionState;
pub use status::StatusManager;
pub use task::{ExecutionContext, ExecutionTask, TaskPipeline};
