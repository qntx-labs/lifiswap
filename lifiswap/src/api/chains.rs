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
        let resp: ChainsResponse = match params.and_then(|p| p.chain_types.as_deref()) {
            Some(types) => {
                let q: Vec<_> = types
                    .iter()
                    .map(|ct| ("chainTypes", ct.to_string()))
                    .collect();
                self.get("/chains", &q).await?
            }
            None => self.get("/chains", &()).await?,
        };
        Ok(resp.chains)
    }
}
