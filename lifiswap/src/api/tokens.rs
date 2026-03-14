//! `GET /tokens` and `GET /token` endpoints.

use crate::client::LiFiClient;
use crate::error::{LiFiError, Result};
use crate::types::{TokenExtended, TokensRequest, TokensResponse};

impl LiFiClient {
    /// Get all known tokens, optionally filtered by chain.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError`] on network or API errors.
    pub async fn get_tokens(&self, params: Option<&TokensRequest>) -> Result<TokensResponse> {
        match params {
            Some(p) => self.get("/tokens", p).await,
            None => self.get("/tokens", &()).await,
        }
    }

    /// Fetch information about a single token.
    ///
    /// # Arguments
    ///
    /// * `chain` — Chain ID or key (e.g. `"1"`, `"eth"`).
    /// * `token` — Token address or symbol.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError::Validation`] if parameters are empty, or
    /// [`LiFiError`] on network/API errors.
    pub async fn get_token(&self, chain: &str, token: &str) -> Result<TokenExtended> {
        if chain.is_empty() {
            return Err(LiFiError::Validation(
                "required parameter \"chain\" is missing".into(),
            ));
        }
        if token.is_empty() {
            return Err(LiFiError::Validation(
                "required parameter \"token\" is missing".into(),
            ));
        }

        self.get("/token", &[("chain", chain), ("token", token)])
            .await
    }
}
