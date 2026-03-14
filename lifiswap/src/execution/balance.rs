//! Token balance query and name service orchestration functions.
//!
//! Mirrors the `TypeScript` SDK's `getTokenBalance`, `getTokenBalances`,
//! `getTokenBalancesByChain`, and `getNameServiceAddress` actions.
//! These are convenience methods on [`LiFiClient`] that route queries
//! to the appropriate chain provider.

use std::collections::HashMap;

use crate::LiFiClient;
use crate::error::{LiFiError, Result};
use crate::provider::Provider;
use crate::types::{ChainId, ChainType, Token, TokenAmount};

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
        providers: &[Box<dyn Provider>],
    ) -> Result<Option<TokenAmount>> {
        let balances = self
            .get_token_balances(wallet_address, &[token.clone()], providers)
            .await?;
        Ok(balances.into_iter().next())
    }

    /// Query balances for a list of tokens across chains for a wallet.
    ///
    /// Tokens are grouped by chain ID and queried concurrently via the
    /// appropriate provider. Results are flattened into a single list.
    ///
    /// # Errors
    ///
    /// Returns an error if no matching provider is found or any RPC call fails.
    pub async fn get_token_balances(
        &self,
        wallet_address: &str,
        tokens: &[Token],
        providers: &[Box<dyn Provider>],
    ) -> Result<Vec<TokenAmount>> {
        let mut tokens_by_chain: HashMap<ChainId, Vec<Token>> = HashMap::new();
        for token in tokens {
            tokens_by_chain
                .entry(token.chain_id)
                .or_default()
                .push(token.clone());
        }

        let results = self
            .get_token_balances_by_chain(wallet_address, &tokens_by_chain, providers)
            .await?;

        Ok(results.into_values().flatten().collect())
    }

    /// Query token balances grouped by chain ID.
    ///
    /// For each chain, finds the matching provider by chain type and
    /// queries balances concurrently. Chains without a matching provider
    /// are silently skipped (matching TS SDK behavior).
    ///
    /// # Errors
    ///
    /// Returns an error if the wallet address is empty.
    pub async fn get_token_balances_by_chain(
        &self,
        wallet_address: &str,
        tokens_by_chain: &HashMap<ChainId, Vec<Token>>,
        providers: &[Box<dyn Provider>],
    ) -> Result<HashMap<ChainId, Vec<TokenAmount>>> {
        if wallet_address.is_empty() {
            return Err(LiFiError::Validation("Missing walletAddress.".to_owned()));
        }

        let provider = providers
            .iter()
            .find(|p| p.is_address(wallet_address))
            .ok_or_else(|| LiFiError::Provider {
                code: crate::error::LiFiErrorCode::ProviderUnavailable,
                message: format!("SDK Token Provider for {wallet_address} is not found."),
            })?;

        let chains = self.get_chains(None).await?;

        let mut result: HashMap<ChainId, Vec<TokenAmount>> = HashMap::new();

        for (&chain_id, tokens) in tokens_by_chain {
            let chain = chains.iter().find(|c| c.id == chain_id);
            let chain_type_matches = chain
                .is_some_and(|c| c.chain_type == provider.chain_type());

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

    /// Resolve a human-readable name (e.g. ENS, SNS) to an on-chain address.
    ///
    /// Tries each provider that matches the optional `chain_type` filter.
    /// Returns the first successful resolution, or `None` if no provider
    /// can resolve the name.
    ///
    /// Mirrors the `TypeScript` SDK's `getNameServiceAddress` action.
    pub async fn get_name_service_address(
        &self,
        name: &str,
        chain_type: Option<ChainType>,
        providers: &[Box<dyn Provider>],
    ) -> Option<String> {
        let filtered: Vec<&dyn Provider> = if let Some(ct) = chain_type {
            providers
                .iter()
                .filter(|p| p.chain_type() == ct)
                .map(AsRef::as_ref)
                .collect()
        } else {
            providers.iter().map(AsRef::as_ref).collect()
        };

        for provider in filtered {
            match provider.resolve_address(name, None).await {
                Ok(Some(address)) => return Some(address),
                Ok(None) | Err(_) => continue,
            }
        }

        None
    }
}
