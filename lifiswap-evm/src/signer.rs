//! EVM signer abstraction for transaction signing and broadcasting.

use std::future::Future;
use std::pin::Pin;

use alloy::network::EthereumWallet;
use alloy::primitives::{Address, B256, Bytes, U256};
use alloy::providers::{Provider as _, ProviderBuilder};
use alloy::rpc::types::{TransactionReceipt, TransactionRequest};
use alloy::signers::Signer as _;
use alloy::signers::local::PrivateKeySigner;
use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::types::TypedData;

/// A single call within an EIP-5792 batch.
#[derive(Debug, Clone)]
pub struct BatchCall {
    /// Target address.
    pub to: Address,
    /// Calldata.
    pub data: Bytes,
    /// Native token value.
    pub value: U256,
}

/// Receipt for a single call within a batch.
#[derive(Debug, Clone, Copy)]
pub struct BatchCallReceipt {
    /// Transaction hash.
    pub tx_hash: B256,
    /// Whether the transaction succeeded.
    pub success: bool,
}

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

    /// Wait for a transaction to be included and return the receipt.
    ///
    /// Uses the signer's RPC connection so no separate provider is needed.
    /// The default implementation returns an error; signers with RPC access
    /// should override this.
    fn confirm_transaction<'a>(
        &'a self,
        _tx_hash: B256,
    ) -> Pin<Box<dyn Future<Output = Result<TransactionReceipt>> + Send + 'a>> {
        Box::pin(async {
            Err(LiFiError::Transaction {
                code: LiFiErrorCode::InternalError,
                message: "This signer does not support transaction confirmation.".to_owned(),
            })
        })
    }

    /// Sign EIP-712 typed data, returning the hex-encoded signature.
    ///
    /// Used for relay/gasless flows where the relayer submits the transaction
    /// on behalf of the user.
    ///
    /// The default implementation returns an error indicating typed data
    /// signing is not supported. Override this for signers that support
    /// relay transactions.
    fn sign_typed_data<'a>(
        &'a self,
        _typed_data: &'a TypedData,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async {
            Err(LiFiError::Transaction {
                code: LiFiErrorCode::InternalError,
                message: "This signer does not support EIP-712 typed data signing.".to_owned(),
            })
        })
    }

    /// Whether this signer supports EIP-5792 batch calls (`wallet_sendCalls`).
    ///
    /// When `true`, the execution pipeline batches approve + swap calls
    /// into a single atomic submission.
    fn supports_batching(&self) -> bool {
        false
    }

    /// Submit a batch of calls via EIP-5792 `wallet_sendCalls`.
    ///
    /// Returns a batch identifier that can be polled via [`get_calls_status`](Self::get_calls_status).
    fn send_calls<'a>(
        &'a self,
        _calls: Vec<BatchCall>,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async {
            Err(LiFiError::Transaction {
                code: LiFiErrorCode::InternalError,
                message: "This signer does not support EIP-5792 batch calls.".to_owned(),
            })
        })
    }

    /// Poll the status of a batch submitted via [`send_calls`](Self::send_calls).
    ///
    /// Returns `Ok(receipts)` once the batch is confirmed. The last receipt
    /// in the list corresponds to the main transaction.
    fn get_calls_status<'a>(
        &'a self,
        _batch_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<BatchCallReceipt>>> + Send + 'a>> {
        Box::pin(async {
            Err(LiFiError::Transaction {
                code: LiFiErrorCode::InternalError,
                message: "This signer does not support EIP-5792 batch status.".to_owned(),
            })
        })
    }
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
    pub const fn new(signer: PrivateKeySigner, rpc_url: url::Url) -> Self {
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

    fn confirm_transaction<'a>(
        &'a self,
        tx_hash: B256,
    ) -> Pin<Box<dyn Future<Output = Result<TransactionReceipt>> + Send + 'a>> {
        Box::pin(async move {
            let provider = ProviderBuilder::new().connect_http(self.rpc_url.clone());
            alloy::providers::PendingTransactionBuilder::new(provider.root().clone(), tx_hash)
                .with_timeout(Some(std::time::Duration::from_secs(240)))
                .get_receipt()
                .await
                .map_err(|e| LiFiError::Transaction {
                    code: LiFiErrorCode::TransactionFailed,
                    message: format!("Failed to get transaction receipt: {e}"),
                })
        })
    }

    fn sign_typed_data<'a>(
        &'a self,
        typed_data: &'a TypedData,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            let json = serde_json::json!({
                "domain": typed_data.domain,
                "types": typed_data.types,
                "primaryType": typed_data.primary_type,
                "message": typed_data.message,
            });

            let payload: alloy::dyn_abi::eip712::TypedData =
                serde_json::from_value(json).map_err(|e| LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: format!("Failed to parse EIP-712 typed data: {e}"),
                })?;

            let sig = self
                .signer
                .sign_dynamic_typed_data(&payload)
                .await
                .map_err(|e| LiFiError::Transaction {
                    code: LiFiErrorCode::TransactionFailed,
                    message: format!("EIP-712 signing failed: {e}"),
                })?;

            Ok(format!("0x{}", alloy::hex::encode(sig.as_bytes())))
        })
    }
}
