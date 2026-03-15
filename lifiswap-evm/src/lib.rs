//! `LiFi` SDK - EVM chain provider (alloy-based).
//!
//! This crate provides an EVM-specific implementation of the [`lifiswap::provider::Provider`]
//! trait, using [alloy](https://docs.rs/alloy) for on-chain interactions.
//!
//! # Example
//!
//! ```ignore
//! use lifiswap::{LiFiClient, LiFiConfig};
//! use lifiswap::execution::execute_route;
//! use lifiswap_evm::{EvmProvider, LocalSigner};
//! use alloy::signers::local::PrivateKeySigner;
//!
//! let key: PrivateKeySigner = "0xac0974...".parse().unwrap();
//! let rpc_url: url::Url = "https://eth.llamarpc.com".parse().unwrap();
//! let signer = LocalSigner::new(key, rpc_url.clone());
//! let provider = EvmProvider::new(signer, rpc_url);
//!
//! let client = LiFiClient::new(LiFiConfig::builder().integrator("my-app").build())?;
//! let route = client.get_routes(&req).await?.routes.remove(0);
//! let result = execute_route(&client, route, &[Box::new(provider)], Default::default()).await?;
//! ```

mod errors;
mod executor;
pub mod permit2;
mod provider;
pub mod rpc;
pub mod signer;
mod tasks;

use alloy::primitives::Address;
pub use executor::Permit2Config;
pub use provider::EvmProvider;
pub use rpc::{RpcUrlResolver, StaticRpcUrls};
pub use signer::{BatchCall, BatchCallReceipt, EvmSigner, LocalSigner};

/// Check whether a token address represents the native token (ETH).
pub(crate) fn is_native_token(address: &str) -> bool {
    address.parse::<Address>().is_ok_and(|a| a.is_zero())
        || address.eq_ignore_ascii_case("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE")
}
