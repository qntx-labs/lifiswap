//! Bitcoin signer abstraction.
//!
//! Defines the [`BtcSigner`] trait for signing PSBTs and a local
//! [`KeypairSigner`] implementation using a private key.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

use bitcoin::key::PrivateKey;
use bitcoin::psbt::Psbt;
use bitcoin::secp256k1::{All, Secp256k1};
use bitcoin::{Address, CompressedPublicKey, Network, PublicKey};
use lifiswap::error::Result;

/// Abstracts Bitcoin PSBT signing, allowing different backends
/// (local keypair, hardware wallet, remote signer, etc.).
pub trait BtcSigner: Send + Sync + 'static {
    /// Returns the signer's Bitcoin address.
    fn address(&self) -> &Address;

    /// Returns the signer's compressed public key (33 bytes).
    fn public_key(&self) -> CompressedPublicKey;

    /// Sign all inputs in a PSBT that belong to this signer.
    fn sign_psbt<'a>(
        &'a self,
        psbt: &'a mut Psbt,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
}

/// Local private-key based Bitcoin signer.
///
/// Uses a `bitcoin::PrivateKey` and `secp256k1::Secp256k1` context
/// for PSBT signing. Suitable for server-side / CLI usage.
///
/// # Example
///
/// ```ignore
/// use bitcoin::key::PrivateKey;
/// use bitcoin::Network;
/// use lifiswap_btc::KeypairSigner;
///
/// let private_key = PrivateKey::generate(Network::Bitcoin);
/// let signer = KeypairSigner::new(private_key, Network::Bitcoin);
/// ```
#[derive(Debug, Clone)]
pub struct KeypairSigner {
    private_key: PrivateKey,
    public_key: CompressedPublicKey,
    address: Address,
    secp: Secp256k1<All>,
}

impl KeypairSigner {
    /// Create a new keypair signer from a private key and network.
    #[must_use]
    pub fn new(private_key: PrivateKey, network: Network) -> Self {
        let secp = Secp256k1::new();
        let secp_pubkey = private_key.inner.public_key(&secp);
        let public_key = CompressedPublicKey(secp_pubkey);
        let address = Address::p2wpkh(&public_key, network);
        Self {
            private_key,
            public_key,
            address,
            secp,
        }
    }

    /// Create a signer from a WIF-encoded private key string.
    ///
    /// # Errors
    ///
    /// Returns an error if the WIF string is invalid.
    pub fn from_wif(wif: &str, network: Network) -> Result<Self> {
        let private_key: PrivateKey = wif.parse().map_err(|e| {
            lifiswap::error::LiFiError::Config(format!("Invalid WIF private key: {e}"))
        })?;
        Ok(Self::new(private_key, network))
    }
}

impl BtcSigner for KeypairSigner {
    fn address(&self) -> &Address {
        &self.address
    }

    fn public_key(&self) -> CompressedPublicKey {
        self.public_key
    }

    fn sign_psbt<'a>(
        &'a self,
        psbt: &'a mut Psbt,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let mut keys = BTreeMap::new();
            keys.insert(PublicKey::new(self.public_key.0), self.private_key);

            psbt.sign(&keys, &self.secp).map_err(|(_, sign_errors)| {
                let msg = sign_errors
                    .iter()
                    .map(|(idx, e)| format!("input {idx}: {e}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                lifiswap::error::LiFiError::Transaction {
                    code: lifiswap::error::LiFiErrorCode::TransactionFailed,
                    message: format!("PSBT signing failed: {msg}"),
                }
            })?;
            Ok(())
        })
    }
}
