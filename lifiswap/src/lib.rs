//! # lifiswap
//!
//! A Rust SDK for the [LI.FI](https://li.fi) cross-chain swap and bridge aggregation API.
//!
//! ## Quick Start
//!
//! ```no_run
//! use lifiswap::{LiFiClient, LiFiConfig};
//!
//! # async fn example() -> lifiswap::error::Result<()> {
//! let client = LiFiClient::new(
//!     LiFiConfig::builder().integrator("my-app").build(),
//! )?;
//!
//! let chains = client.get_chains(None).await?;
//! println!("supported chains: {}", chains.len());
//! # Ok(())
//! # }
//! ```
//!
//! ## Builder Pattern
//!
//! All request types support ergonomic builders via [`bon`]:
//!
//! ```no_run
//! use lifiswap::{LiFiClient, LiFiConfig};
//! use lifiswap::types::QuoteRequest;
//!
//! # async fn example() -> lifiswap::error::Result<()> {
//! let client = LiFiClient::new(
//!     LiFiConfig::builder()
//!         .integrator("my-app")
//!         .api_key("sk-...")
//!         .build(),
//! )?;
//!
//! let quote = client
//!     .get_quote(
//!         &QuoteRequest::builder()
//!             .from_chain("1")
//!             .from_token("0xUSDC...")
//!             .from_address("0xYourWallet...")
//!             .from_amount("1000000")
//!             .to_chain("137")
//!             .to_token("0xUSDC_POL...")
//!             .build(),
//!     )
//!     .await?;
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod error;
pub mod types;

mod api;
mod http;

pub use client::{LiFiClient, LiFiConfig, RetryConfig};
