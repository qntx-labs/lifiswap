//! `GET /chains` endpoint.

use crate::client::LiFiClient;
use crate::error::Result;
use crate::http;
use crate::types::{ChainsRequest, ChainsResponse, ExtendedChain};

impl LiFiClient {
    /// Get all available chains.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError`](crate::error::LiFiError) on network or API errors.
    pub async fn get_chains(&self, params: Option<&ChainsRequest>) -> Result<Vec<ExtendedChain>> {
        let cfg = self.http_config();
        let mut url = format!("{}/chains", cfg.api_url);

        if let Some(p) = params {
            let mut qs = Vec::new();
            if let Some(ref chain_types) = p.chain_types {
                for ct in chain_types {
                    qs.push(format!("chainTypes={ct}"));
                }
            }
            if !qs.is_empty() {
                url = format!("{url}?{}", qs.join("&"));
            }
        }

        let resp: ChainsResponse = http::get(&self.http, &cfg, &url).await?;
        Ok(resp.chains)
    }
}
