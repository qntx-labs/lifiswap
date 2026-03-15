//! EVM signer abstraction for transaction signing and broadcasting.

use std::future::Future;
use std::pin::Pin;

use alloy::network::EthereumWallet;
use alloy::primitives::{Address, B256};
use alloy::providers::{Provider as _, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use lifiswap::error::{LiFiError, LiFiErrorCode, Result};

/// Abstraction for EVM transaction signing and broadcasting.
///
/// Implementations handle the details of signing transactions and
/// submitting them to the network. The core SDK is agnostic to the
/// signing backend (private key, hardware wallet, browser extension, etc.).
///
/// # Implementing a custom signer
///
/// ```ignore
/// use lifiswap_evm::signer::EvmSigner;
///
/// struct MySigner { /* ... */ }
///
/// impl EvmSigner for MySigner {
///     fn address(&self) -> Address { /* ... */ }
///     fn send_transaction<'a>(&'a self, tx: TransactionRequest)
///         -> Pin<Box<dyn Future<Output = Result<B256>> + Send + 'a>>
///     {
///         Box::pin(async move { /* sign and broadcast */ })
///     }
/// }
/// ```
pub trait EvmSigner: Send + Sync + std::fmt::Debug + 'static {
    /// Returns the wallet address.
    fn address(&self) -> Address;

    /// Sign and broadcast a transaction, returning the tx hash.
    ///
    /// The transaction is considered submitted once this future resolves.
    /// Receipt confirmation is handled separately by the execution pipeline.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError::Transaction`] if signing or broadcasting fails.
    fn send_transaction<'a>(
        &'a self,
        tx: TransactionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<B256>> + Send + 'a>>;
}

/// Local private-key signer for backend/CLI usage.
///
/// Signs transactions locally and broadcasts via the configured RPC endpoint.
#[derive(Debug, Clone)]
pub struct LocalSigner {
    signer: PrivateKeySigner,
    rpc_url: url::Url,
}

impl LocalSigner {
    /// Create a new local signer.
    #[must_use]
    pub fn new(signer: PrivateKeySigner, rpc_url: url::Url) -> Self {
        Self { signer, rpc_url }
    }
}

impl EvmSigner for LocalSigner {
    fn address(&self) -> Address {
        self.signer.address()
    }

    fn send_transaction<'a>(
        &'a self,
        tx: TransactionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<B256>> + Send + 'a>> {
        Box::pin(async move {
            let wallet = EthereumWallet::from(self.signer.clone());
            let provider = ProviderBuilder::new()
                .wallet(wallet)
                .connect_http(self.rpc_url.clone());

            let pending =
                provider
                    .send_transaction(tx)
                    .await
                    .map_err(|e| LiFiError::Transaction {
                        code: LiFiErrorCode::TransactionFailed,
                        message: format!("Send transaction failed: {e}"),
                    })?;

            Ok(*pending.tx_hash())
        })
    }
}
