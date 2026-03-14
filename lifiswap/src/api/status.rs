//! `GET /status` endpoint.

use crate::client::LiFiClient;
use crate::error::{LiFiError, Result};
use crate::http;
use crate::types::{StatusRequest, StatusResponse};

impl LiFiClient {
    /// Check the status of a transfer.
    ///
    /// For cross-chain transfers, the `bridge` parameter is recommended.
    /// Either `tx_hash` or `task_id` must be provided.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError::Validation`] if neither `tx_hash` nor `task_id` is set,
    /// or [`LiFiError`] on network/API errors.
    pub async fn get_status(&self, params: &StatusRequest) -> Result<StatusResponse> {
        if params.tx_hash.is_none() && params.task_id.is_none() {
            return Err(LiFiError::Validation(
                "either \"txHash\" or \"taskId\" must be provided".into(),
            ));
        }

        let cfg = self.http_config();
        let mut query: Vec<(&str, String)> = Vec::new();

        if let Some(ref h) = params.tx_hash {
            query.push(("txHash", h.clone()));
        }
        if let Some(ref t) = params.task_id {
            query.push(("taskId", t.clone()));
        }
        if let Some(ref b) = params.bridge {
            query.push(("bridge", b.clone()));
        }
        if let Some(ref fc) = params.from_chain {
            query.push(("fromChain", fc.to_string()));
        }
        if let Some(ref tc) = params.to_chain {
            query.push(("toChain", tc.to_string()));
        }

        let base = url::Url::parse(&format!("{}/status", cfg.api_url))?;
        let pairs: Vec<(&str, &str)> = query.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let url = url::Url::parse_with_params(base.as_str(), &pairs)?;

        http::get(&self.http, &cfg, url.as_str()).await
    }
}
