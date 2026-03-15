//! Bitcoin chain provider implementation.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use bitcoin::Address;
use lifiswap::error::Result;
use lifiswap::provider::{Provider, StepExecutor};
use lifiswap::types::{ChainType, StepExecutorOptions, Token, TokenAmount};

use crate::api::BlockchainApi;
use crate::executor::BtcStepExecutor;
use crate::signer::BtcSigner;

/// Bitcoin chain provider using the [`bitcoin`] crate and public REST APIs.
///
/// Handles address validation, balance queries, and creates
/// [`BtcStepExecutor`] instances for step execution.
///
/// # Example
///
/// ```ignore
/// use bitcoin::key::PrivateKey;
/// use bitcoin::Network;
/// use lifiswap_btc::{BtcProvider, KeypairSigner};
///
/// let key = PrivateKey::generate(Network::Bitcoin);
/// let signer = KeypairSigner::new(key, Network::Bitcoin);
/// let provider = BtcProvider::new(signer);
/// ```
#[derive(Clone)]
pub struct BtcProvider {
    signer: Arc<dyn BtcSigner>,
    api: BlockchainApi,
}

impl std::fmt::Debug for BtcProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BtcProvider")
            .field("address", &self.signer.address())
            .finish_non_exhaustive()
    }
}

impl BtcProvider {
    /// Create a new Bitcoin provider with the default mempool.space API.
    #[must_use]
    pub fn new(signer: impl BtcSigner) -> Self {
        Self {
            signer: Arc::new(signer),
            api: BlockchainApi::new(),
        }
    }

    /// Create a provider with a custom blockchain API client.
    #[must_use]
    pub fn with_api(signer: impl BtcSigner, api: BlockchainApi) -> Self {
        Self {
            signer: Arc::new(signer),
            api,
        }
    }

    /// Returns the signer's Bitcoin address.
    #[must_use]
    pub fn address(&self) -> &Address {
        self.signer.address()
    }
}

impl Provider for BtcProvider {
    fn chain_type(&self) -> ChainType {
        ChainType::UTXO
    }

    fn is_address(&self, address: &str) -> bool {
        address.parse::<Address<_>>().is_ok()
    }

    fn resolve_address<'a>(
        &'a self,
        name: &'a str,
        _chain_id: Option<u64>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + 'a>> {
        // Bitcoin does not support domain name resolution — pass through as-is
        Box::pin(async { Ok(Some(name.to_owned())) })
    }

    fn get_balance<'a>(
        &'a self,
        wallet_address: &'a str,
        tokens: &'a [Token],
    ) -> Pin<Box<dyn Future<Output = Result<Vec<TokenAmount>>> + Send + 'a>> {
        Box::pin(async move {
            if tokens.is_empty() {
                return Ok(vec![]);
            }

            let balance = self.api.get_balance(wallet_address).await?;
            let block_height = self.api.get_block_height().await.ok();

            Ok(tokens
                .iter()
                .map(|token| TokenAmount {
                    token: token.clone(),
                    amount: Some(balance.to_string()),
                    block_number: block_height,
                })
                .collect())
        })
    }

    fn create_step_executor<'a>(
        &'a self,
        options: StepExecutorOptions,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn StepExecutor>>> + Send + 'a>> {
        Box::pin(async move {
            let executor: Box<dyn StepExecutor> = Box::new(BtcStepExecutor::new(
                Arc::clone(&self.signer),
                self.api.clone(),
                options,
            ));
            Ok(executor)
        })
    }
}
