//! `GET /wallets/{address}/balances` endpoint.

use std::collections::HashMap;

use crate::client::LiFiClient;
use crate::error::{LiFiError, Result};
use crate::types::{WalletBalancesResponse, WalletTokenExtended};

impl LiFiClient {
    /// Get token balances for a wallet across all supported chains.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError::Validation`] if `wallet_address` is empty, or
    /// [`LiFiError`] on network/API errors.
    pub async fn get_wallet_balances(
        &self,
        wallet_address: &str,
    ) -> Result<HashMap<u64, Vec<WalletTokenExtended>>> {
        if wallet_address.is_empty() {
            return Err(LiFiError::Validation("missing walletAddress".into()));
        }

        let path = format!("/wallets/{wallet_address}/balances");
        let resp: WalletBalancesResponse = self.get(&path, &[("extended", "true")]).await?;
        Ok(resp.balances)
    }
}
