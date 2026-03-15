//! `LiFi` SDK - Bitcoin chain provider.
//!
//! This crate provides a Bitcoin-specific implementation of the [`lifiswap::Provider`] trait
//! for UTXO-based Bitcoin transactions using PSBT (BIP-174) signing.
//!
//! # Architecture
//!
//! - [`BtcProvider`] — implements [`lifiswap::Provider`] for Bitcoin (ChainType::UTXO)
//! - [`BtcSigner`] — trait abstracting PSBT signing (local keypair, hardware wallet, etc.)
//! - [`KeypairSigner`] — local private-key based signer for server/CLI usage
//! - [`BlockchainApi`] — REST API client for mempool.space with multi-backend fallback
//!
//! # Example
//!
//! ```ignore
//! use bitcoin::key::PrivateKey;
//! use bitcoin::Network;
//! use lifiswap_btc::{BtcProvider, KeypairSigner};
//!
//! let key = PrivateKey::generate(Network::Bitcoin);
//! let signer = KeypairSigner::new(key);
//! let provider = BtcProvider::new(signer);
//! ```

mod api;
mod errors;
mod executor;
mod provider;
mod signer;
mod tasks;

pub use api::BlockchainApi;
pub use provider::BtcProvider;
pub use signer::{BtcSigner, KeypairSigner};
