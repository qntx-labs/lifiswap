//! Solana signer abstraction for transaction signing.

use std::future::Future;
use std::pin::Pin;

use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::VersionedTransaction;

/// Abstraction for Solana transaction signing.
///
/// Implementations handle the details of signing serialized transactions.
/// The core SDK is agnostic to the signing backend (local keypair,
/// hardware wallet, browser extension, etc.).
///
/// # Implementing a custom signer
///
/// ```ignore
/// use lifiswap_svm::SvmSigner;
///
/// struct MySigner { /* ... */ }
///
/// impl SvmSigner for MySigner {
///     fn pubkey(&self) -> Pubkey { /* ... */ }
///     fn sign_transactions<'a>(&'a self, txs: Vec<VersionedTransaction>)
///         -> Pin<Box<dyn Future<Output = Result<Vec<VersionedTransaction>>> + Send + 'a>>
///     {
///         Box::pin(async move { /* sign and return */ })
///     }
/// }
/// ```
pub trait SvmSigner: Send + Sync + std::fmt::Debug + 'static {
    /// Returns the wallet public key.
    fn pubkey(&self) -> Pubkey;

    /// Sign one or more versioned transactions.
    ///
    /// The transactions arrive partially-constructed from the API (with a
    /// recent blockhash already set). The signer adds its signature(s) and
    /// returns the fully-signed transactions.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError::Transaction`] if signing fails.
    fn sign_transactions<'a>(
        &'a self,
        txs: Vec<VersionedTransaction>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<VersionedTransaction>>> + Send + 'a>>;
}

/// Local keypair signer for backend / CLI usage.
///
/// Signs transactions locally using an in-memory [`Keypair`].
///
/// # Security
///
/// The secret key is held in process memory. This is suitable for server-side
/// or CLI applications but **not** for browser/frontend usage.
pub struct KeypairSigner {
    keypair: Keypair,
}

impl std::fmt::Debug for KeypairSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeypairSigner")
            .field("pubkey", &self.keypair.pubkey())
            .finish_non_exhaustive()
    }
}

impl KeypairSigner {
    /// Create a new keypair signer from a [`Keypair`].
    #[must_use]
    pub const fn new(keypair: Keypair) -> Self {
        Self { keypair }
    }

    /// Create a signer from a base58-encoded secret key.
    ///
    /// # Errors
    ///
    /// Returns an error if the key cannot be decoded or is invalid.
    pub fn from_base58(secret_key: &str) -> Result<Self> {
        let keypair = Keypair::try_from_base58_string(secret_key)
            .map_err(|e| LiFiError::Validation(format!("Invalid base58 keypair: {e}")))?;
        Ok(Self { keypair })
    }

    /// Returns a reference to the inner keypair.
    #[must_use]
    pub const fn keypair(&self) -> &Keypair {
        &self.keypair
    }
}

impl SvmSigner for KeypairSigner {
    fn pubkey(&self) -> Pubkey {
        self.keypair.pubkey()
    }

    fn sign_transactions<'a>(
        &'a self,
        txs: Vec<VersionedTransaction>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<VersionedTransaction>>> + Send + 'a>> {
        Box::pin(async move {
            let mut signed = Vec::with_capacity(txs.len());
            for mut tx in txs {
                let message_data = tx.message.serialize();
                let sig = self.keypair.sign_message(&message_data);

                let pubkey = self.keypair.pubkey();
                let position = tx
                    .message
                    .static_account_keys()
                    .iter()
                    .position(|k| k == &pubkey)
                    .ok_or_else(|| LiFiError::Transaction {
                        code: LiFiErrorCode::TransactionFailed,
                        message: format!(
                            "Signer pubkey {pubkey} not found in transaction account keys"
                        ),
                    })?;

                if position < tx.signatures.len() {
                    tx.signatures[position] = sig;
                } else {
                    return Err(LiFiError::Transaction {
                        code: LiFiErrorCode::TransactionFailed,
                        message: "Signature index out of bounds".to_owned(),
                    });
                }

                signed.push(tx);
            }
            Ok(signed)
        })
    }
}
