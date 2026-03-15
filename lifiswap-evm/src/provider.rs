//! EVM chain provider implementation.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use alloy::ens::ProviderEnsExt as _;
use alloy::primitives::Address;
use alloy::providers::{Provider as AlloyProvider, ProviderBuilder};
use alloy::sol;
use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::provider::{Provider, StepExecutor};
use lifiswap::types::{ChainType, StepExecutorOptions, Token, TokenAmount};

use crate::executor::{EvmStepExecutor, Permit2Config};
use crate::rpc::RpcUrlResolver;
use crate::signer::EvmSigner;

sol! {
    #[sol(rpc)]
    contract IERC20Balance {
        function balanceOf(address account) external view returns (uint256);
    }
}

/// EVM chain provider using [`alloy`] for on-chain interactions.
///
/// Handles address validation, balance queries, and creates
/// [`EvmStepExecutor`] instances for step execution.
///
/// The signing backend is abstracted via [`EvmSigner`], allowing
/// different backends (local private key, hardware wallet, etc.).
#[derive(Clone)]
pub struct EvmProvider {
    signer: Arc<dyn EvmSigner>,
    rpc_url: url::Url,
    rpc_resolver: Option<Arc<dyn RpcUrlResolver>>,
    permit2: Option<Permit2Config>,
}

impl std::fmt::Debug for EvmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvmProvider")
            .field("address", &self.signer.address())
            .field("rpc_url", &self.rpc_url.as_str())
            .field("rpc_resolver", &self.rpc_resolver)
            .finish_non_exhaustive()
    }
}

impl EvmProvider {
    /// Create a new EVM provider with the given signer and RPC URL.
    ///
    /// The `rpc_url` is used for read-only operations (balance queries, allowance checks).
    /// The signer handles transaction signing and broadcasting independently.
    #[must_use]
    pub fn new(signer: impl EvmSigner, rpc_url: url::Url) -> Self {
        Self {
            signer: Arc::new(signer),
            rpc_url,
            rpc_resolver: None,
            permit2: None,
        }
    }

    /// Attach an [`RpcUrlResolver`] for multi-chain RPC endpoint resolution.
    ///
    /// When set, `get_balance` and other read operations will use the resolver
    /// to find the appropriate RPC endpoint for a given chain ID, falling back
    /// to the default `rpc_url` if the resolver returns `None`.
    #[must_use]
    pub fn with_rpc_resolver(mut self, resolver: impl RpcUrlResolver) -> Self {
        self.rpc_resolver = Some(Arc::new(resolver));
        self
    }

    /// Enable Permit2 support with the given contract addresses.
    ///
    /// Addresses are typically obtained from the `/chains` API response
    /// (`Chain.permit2` and `Chain.permit2_proxy`).
    #[must_use]
    pub const fn with_permit2(mut self, permit2: Address, permit2_proxy: Address) -> Self {
        self.permit2 = Some(Permit2Config {
            permit2,
            permit2_proxy,
        });
        self
    }

    /// Returns the wallet address derived from the signer.
    #[must_use]
    pub fn address(&self) -> Address {
        self.signer.address()
    }

    /// Resolve the RPC URL for a given chain, falling back to the default.
    fn rpc_for_chain(&self, chain_id: u64) -> url::Url {
        self.rpc_resolver
            .as_ref()
            .and_then(|r| r.resolve(chain_id))
            .unwrap_or_else(|| self.rpc_url.clone())
    }
}

impl Provider for EvmProvider {
    fn chain_type(&self) -> ChainType {
        ChainType::EVM
    }

    fn is_address(&self, address: &str) -> bool {
        address.parse::<Address>().is_ok()
    }

    fn resolve_address<'a>(
        &'a self,
        name: &'a str,
        _chain_id: Option<u64>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + 'a>> {
        Box::pin(async move {
            if !name.contains('.') {
                return Ok(None);
            }

            let rpc = self.rpc_for_chain(1);
            let provider = ProviderBuilder::new().connect_http(rpc);

            Ok(provider
                .resolve_name(name)
                .await
                .ok()
                .map(|addr| format!("{addr:#x}")))
        })
    }

    fn get_balance<'a>(
        &'a self,
        wallet_address: &'a str,
        tokens: &'a [Token],
    ) -> Pin<Box<dyn Future<Output = Result<Vec<TokenAmount>>> + Send + 'a>> {
        Box::pin(async move {
            let addr: Address = wallet_address.parse().map_err(|_| {
                LiFiError::Validation(format!("Invalid EVM address: {wallet_address}"))
            })?;

            let chain_id = tokens.first().map_or(0, |t| t.chain_id.0);
            let rpc = self.rpc_for_chain(chain_id);
            let provider = ProviderBuilder::new().connect_http(rpc);

            let native_balance =
                provider
                    .get_balance(addr)
                    .await
                    .map_err(|e| LiFiError::Provider {
                        code: LiFiErrorCode::ProviderUnavailable,
                        message: format!("Failed to fetch native balance: {e}"),
                    })?;

            let mut results = Vec::with_capacity(tokens.len());

            for token in tokens {
                let is_native = token.address == "0x0000000000000000000000000000000000000000"
                    || token.address.to_lowercase() == "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

                let amount = if is_native {
                    Some(native_balance.to_string())
                } else {
                    match token.address.parse::<Address>() {
                        Ok(token_addr) => {
                            let contract = IERC20Balance::new(token_addr, &provider);
                            match contract.balanceOf(addr).call().await {
                                Ok(bal) => Some(bal.to_string()),
                                Err(e) => {
                                    tracing::warn!(
                                        token = %token.symbol,
                                        address = %token.address,
                                        error = %e,
                                        "failed to query ERC-20 balance, skipping"
                                    );
                                    None
                                }
                            }
                        }
                        Err(_) => None,
                    }
                };

                results.push(TokenAmount {
                    token: token.clone(),
                    amount,
                    block_number: None,
                });
            }

            Ok(results)
        })
    }

    fn create_step_executor<'a>(
        &'a self,
        options: StepExecutorOptions,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn StepExecutor>>> + Send + 'a>> {
        Box::pin(async move {
            let executor: Box<dyn StepExecutor> = Box::new(EvmStepExecutor::new(
                Arc::clone(&self.signer),
                self.rpc_url.clone(),
                options,
                self.permit2,
            ));
            Ok(executor)
        })
    }
}
