//! `LiFi` SDK - Solana chain provider.
//!
//! This crate provides a Solana-specific implementation of the
//! [`lifiswap::provider::Provider`] trait, using
//! [`solana-sdk`](https://docs.rs/solana-sdk) for on-chain interactions.
//!
//! # Example
//!
//! ```ignore
//! use lifiswap::{LiFiClient, LiFiConfig};
//! use lifiswap_svm::{SvmProvider, KeypairSigner};
//! use solana_sdk::signature::Keypair;
//!
//! let keypair = Keypair::new();
//! let signer = KeypairSigner::new(keypair);
//! let rpc_url: url::Url = "https://api.mainnet-beta.solana.com".parse().unwrap();
//! let provider = SvmProvider::new(signer, rpc_url);
//!
//! let client = LiFiClient::new(LiFiConfig::builder().integrator("my-app").build())?;
//! client.add_provider(Box::new(provider));
//! ```

mod errors;
mod executor;
pub mod jito;
mod provider;
pub mod rpc;
pub mod signer;
mod tasks;

pub use jito::JitoClient;
pub use provider::SvmProvider;
pub use signer::{KeypairSigner, SvmSigner};
