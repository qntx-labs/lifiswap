//! REST API endpoint implementations.
//!
//! Each submodule corresponds to a `LiFi` API endpoint. All methods are exposed
//! as inherent `impl` blocks on [`LiFiClient`](crate::LiFiClient).

mod chains;
mod connections;
mod gas;
mod patcher;
mod quote;
mod relay;
mod routes;
mod status;
mod step_transaction;
mod tokens;
mod tools;
mod wallet;
