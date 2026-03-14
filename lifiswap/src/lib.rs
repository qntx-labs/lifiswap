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
//! eprintln!("supported chains: {}", chains.len());
//! # Ok(())
//! # }
//! ```
//!
//! ## One-Line Swap
//!
//! The simplest way to perform a cross-chain swap — one method call does everything:
//! fetch the optimal quote, convert it to a route, and execute it end-to-end.
//!
//! ```ignore
//! use lifiswap::{LiFiClient, LiFiConfig};
//! use lifiswap::types::QuoteRequest;
//!
//! let client = LiFiClient::new(
//!     LiFiConfig::builder().integrator("my-app").build(),
//! )?;
//! client.add_provider(evm_provider);
//!
//! let result = client
//!     .swap(
//!         &QuoteRequest::builder()
//!             .from_chain("42161")           // Arbitrum
//!             .from_token("0xaf88d065...")    // USDC
//!             .from_address("0xYourWallet")
//!             .from_amount("10000000")        // 10 USDC
//!             .to_chain("10")                 // Optimism
//!             .to_token("0xDA10009c...")      // DAI
//!             .build(),
//!         Default::default(),
//!     )
//!     .await?;
//! ```
//!
//! ## Step-by-Step Control
//!
//! For more control, break the flow into individual steps:
//!
//! ```ignore
//! // 1. Get a quote
//! let quote = client.get_quote(&request).await?;
//!
//! // 2. Execute the quote directly
//! let result = client.execute_quote(quote, Default::default()).await?;
//!
//! // Or: get multiple routes and pick one
//! let routes = client.get_routes(&routes_request).await?;
//! let result = client.execute_route(routes.routes[0].clone(), Default::default()).await?;
//! ```

pub mod actions;
pub mod client;
pub mod error;
pub mod execution;
pub mod provider;
pub mod types;

mod api;
mod http;

pub use client::{LiFiClient, LiFiConfig, RetryConfig};
