//! Token balance query orchestration.
//!
//! Mirrors the `TypeScript` SDK's `getTokenBalance`, `getTokenBalances`,
//! and `getTokenBalancesByChain` actions.

use std::collections::HashMap;

use crate::LiFiClient;
use crate::error::{LiFiError, LiFiErrorCode, Result};
use crate::types::{ChainId, Token, TokenAmount};

impl LiFiClient {
    /// Query the balance of a single token for a wallet address.
    ///
    /// Finds the appropriate provider and queries on-chain balance.
    /// Returns `None` if the provider returns no data for this token.
    ///
    /// # Errors
    ///
    /// Returns an error if no matching provider is found or the RPC call fails.
    pub async fn get_token_balance(
        &self,
        wallet_address: &str,
        token: &Token,
    ) -> Result<Option<TokenAmount>> {
        let balances = self
            .get_token_balances(wallet_address, std::slice::from_ref(token))
            .await?;
        Ok(balances.into_iter().next())
    }

    /// Query balances for a list of tokens across chains for a wallet.
    ///
    /// Tokens are grouped by chain ID and queried via the appropriate
    /// provider. Results are flattened into a single list.
    ///
    /// # Errors
    ///
    /// Returns an error if no matching provider is found or any RPC call fails.
    pub async fn get_token_balances(
        &self,
        wallet_address: &str,
        tokens: &[Token],
    ) -> Result<Vec<TokenAmount>> {
        let mut tokens_by_chain: HashMap<ChainId, Vec<Token>> = HashMap::new();
        for token in tokens {
            tokens_by_chain
                .entry(token.chain_id)
                .or_default()
                .push(token.clone());
        }

        let results = self
            .get_token_balances_by_chain(wallet_address, &tokens_by_chain)
            .await?;

        Ok(results.into_values().flatten().collect())
    }

    /// Query token balances grouped by chain ID.
    ///
    /// For each chain, finds the matching provider by chain type and
    /// queries balances. Chains without a matching provider are silently
    /// skipped (matching TS SDK behavior).
    ///
    /// # Errors
    ///
    /// Returns an error if the wallet address is empty.
    pub async fn get_token_balances_by_chain(
        &self,
        wallet_address: &str,
        tokens_by_chain: &HashMap<ChainId, Vec<Token>>,
    ) -> Result<HashMap<ChainId, Vec<TokenAmount>>> {
        if wallet_address.is_empty() {
            return Err(LiFiError::Validation("Missing walletAddress.".to_owned()));
        }

        let wallet_addr = wallet_address.to_owned();
        let provider = self
            .find_provider(|p| p.is_address(&wallet_addr))
            .ok_or_else(|| LiFiError::Provider {
                code: LiFiErrorCode::ProviderUnavailable,
                message: format!("SDK Token Provider for {wallet_address} is not found."),
            })?;

        let chains = self.get_chains(None).await?;
        let provider_chain_type = provider.chain_type();

        let mut result: HashMap<ChainId, Vec<TokenAmount>> = HashMap::new();

        for (&chain_id, tokens) in tokens_by_chain {
            let chain_type_matches = chains
                .iter()
                .find(|c| c.id == chain_id)
                .is_some_and(|c| c.chain_type == provider_chain_type);

            if !chain_type_matches {
                continue;
            }

            match provider.get_balance(wallet_address, tokens).await {
                Ok(amounts) => {
                    result.insert(chain_id, amounts);
                }
                Err(e) => {
                    tracing::warn!(
                        chain_id = ?chain_id,
                        error = %e,
                        "couldn't fetch token balance"
                    );
                }
            }
        }

        Ok(result)
    }
}
