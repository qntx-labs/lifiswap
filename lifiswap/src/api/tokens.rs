//! `GET /tokens` and `GET /token` endpoints.

use crate::client::LiFiClient;
use crate::error::{LiFiError, Result};
use crate::http;
use crate::types::{TokenExtended, TokensRequest, TokensResponse};

impl LiFiClient {
    /// Get all known tokens, optionally filtered by chain.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError`] on network or API errors.
    pub async fn get_tokens(&self, params: Option<&TokensRequest>) -> Result<TokensResponse> {
        let cfg = self.http_config();
        let mut url = format!("{}/tokens", cfg.api_url);

        if let Some(p) = params {
            let mut qs = Vec::new();
            if let Some(ref chains) = p.chains {
                qs.push(format!("chains={chains}"));
            }
            if let Some(ref ct) = p.chain_types {
                qs.push(format!("chainTypes={ct}"));
            }
            if let Some(ext) = p.extended {
                qs.push(format!("extended={ext}"));
            }
            if !qs.is_empty() {
                url = format!("{url}?{}", qs.join("&"));
            }
        }

        http::get(&self.http, &cfg, &url).await
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

        let cfg = self.http_config();
        let base = url::Url::parse(&format!("{}/token", cfg.api_url))?;
        let url =
            url::Url::parse_with_params(base.as_str(), &[("chain", chain), ("token", token)])?;

        http::get(&self.http, &cfg, url.as_str()).await
    }
}
