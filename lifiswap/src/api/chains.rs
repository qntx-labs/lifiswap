//! `GET /chains` endpoint.

use serde::Serialize;

use crate::client::LiFiClient;
use crate::error::Result;
use crate::types::{ChainsRequest, ChainsResponse, ExtendedChain};

/// Empty query — used when no params are supplied.
#[derive(Serialize)]
struct Empty;

impl LiFiClient {
    /// Get all available chains.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError`](crate::error::LiFiError) on network or API errors.
    pub async fn get_chains(&self, params: Option<&ChainsRequest>) -> Result<Vec<ExtendedChain>> {
        let resp: ChainsResponse = match params {
            Some(p) => self.get("/chains", p).await?,
            None => self.get("/chains", &Empty).await?,
        };
        Ok(resp.chains)
    }
}
