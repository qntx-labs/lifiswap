//! `GET /chains` endpoint.

use crate::client::LiFiClient;
use crate::error::Result;
use crate::types::{ChainsRequest, ChainsResponse, ExtendedChain};

impl LiFiClient {
    /// Get all available chains.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError`](crate::error::LiFiError) on network or API errors.
    pub async fn get_chains(&self, params: Option<&ChainsRequest>) -> Result<Vec<ExtendedChain>> {
        let mut url = url::Url::parse(&format!("{}/chains", self.api_url()))?;

        if let Some(p) = params
            && let Some(ref types) = p.chain_types
        {
            let mut qs = url.query_pairs_mut();
            for ct in types {
                qs.append_pair("chainTypes", &ct.to_string());
            }
        }

        let resp: ChainsResponse = self.get_url(&url).await?;
        Ok(resp.chains)
    }
}
