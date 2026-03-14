//! # lifiswap
//!
//! A Rust SDK for the [LI.FI](https://li.fi) cross-chain swap and bridge aggregation API.
//!
//! ## Quick Start
//!
//! ```no_run
//! use lifiswap::LiFiClient;
//!
//! # async fn example() -> lifiswap::error::Result<()> {
//! let client = LiFiClient::builder()
//!     .integrator("my-app")
//!     .build()?;
//!
//! let chains = client.get_chains(None).await?;
//! println!("supported chains: {}", chains.len());
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod error;
pub mod types;

mod api;
mod http;

pub use client::{LiFiClient, LiFiClientBuilder, LiFiConfig};
