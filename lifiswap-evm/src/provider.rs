//! EVM chain provider implementation.

use std::future::Future;
use std::pin::Pin;

use alloy::network::EthereumWallet;
use alloy::primitives::Address;
use alloy::providers::{Provider as AlloyProvider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use alloy::sol;
use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::provider::{Provider, StepExecutor};
use lifiswap::types::{ChainType, StepExecutorOptions, Token, TokenAmount};

use crate::executor::EvmStepExecutor;

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
#[derive(Debug, Clone)]
pub struct EvmProvider {
    signer: PrivateKeySigner,
    rpc_url: String,
}

impl EvmProvider {
    /// Create a new EVM provider with the given signer and RPC URL.
    #[must_use]
    pub fn new(signer: PrivateKeySigner, rpc_url: impl Into<String>) -> Self {
        Self {
            signer,
            rpc_url: rpc_url.into(),
        }
    }

    /// Returns the wallet address derived from the signer.
    #[must_use]
    pub const fn address(&self) -> Address {
        self.signer.address()
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
        _name: &'a str,
        _chain_id: Option<u64>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + 'a>> {
        Box::pin(async {
            // ENS resolution could be added here in the future
            Ok(None)
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

            let rpc_url: url::Url = self
                .rpc_url
                .parse()
                .map_err(|e| LiFiError::Config(format!("Invalid RPC URL: {e}")))?;
            let provider = ProviderBuilder::new().connect_http(rpc_url);

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
            let wallet = EthereumWallet::from(self.signer.clone());

            let executor: Box<dyn StepExecutor> =
                Box::new(EvmStepExecutor::new(wallet, self.rpc_url.clone(), options));
            Ok(executor)
        })
    }
}
