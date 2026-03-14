//! EVM chain provider implementation.

use alloy::network::EthereumWallet;
use alloy::primitives::Address;
use alloy::providers::{Provider as AlloyProvider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use alloy::sol;
use async_trait::async_trait;
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

#[async_trait]
impl Provider for EvmProvider {
    fn chain_type(&self) -> ChainType {
        ChainType::EVM
    }

    fn is_address(&self, address: &str) -> bool {
        address.parse::<Address>().is_ok()
    }

    async fn resolve_address(&self, _name: &str, _chain_id: Option<u64>) -> Result<Option<String>> {
        // ENS resolution could be added here in the future
        Ok(None)
    }

    async fn get_balance(
        &self,
        wallet_address: &str,
        tokens: &[Token],
    ) -> Result<Vec<TokenAmount>> {
        let addr: Address = wallet_address
            .parse()
            .map_err(|_| LiFiError::Validation(format!("Invalid EVM address: {wallet_address}")))?;

        let rpc_url: url::Url = self
            .rpc_url
            .parse()
            .map_err(|e| LiFiError::Config(format!("Invalid RPC URL: {e}")))?;
        let provider = ProviderBuilder::new().connect_http(rpc_url);

        let native_balance = provider
            .get_balance(addr)
            .await
            .map_err(|e| LiFiError::Provider {
                code: LiFiErrorCode::RpcError,
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
                address: token.address.clone(),
                decimals: token.decimals,
                symbol: token.symbol.clone(),
                chain_id: token.chain_id,
                coin_key: token.coin_key.clone(),
                name: token.name.clone(),
                logo_uri: token.logo_uri.clone(),
                price_usd: token.price_usd.clone(),
                amount,
                block_number: None,
            });
        }

        Ok(results)
    }

    async fn create_step_executor(
        &self,
        options: StepExecutorOptions,
    ) -> Result<Box<dyn StepExecutor>> {
        let wallet = EthereumWallet::from(self.signer.clone());

        Ok(Box::new(EvmStepExecutor::new(
            wallet,
            self.rpc_url.clone(),
            options,
        )))
    }
}
