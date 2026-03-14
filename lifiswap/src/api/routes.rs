//! `POST /advanced/routes` endpoint.

use crate::client::LiFiClient;
use crate::error::Result;
use crate::http;
use crate::types::{RoutesRequest, RoutesResponse};

impl LiFiClient {
    /// Get available routes for a token transfer.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError`](crate::error::LiFiError) on network or API errors.
    pub async fn get_routes(&self, params: &RoutesRequest) -> Result<RoutesResponse> {
        let cfg = self.http_config();
        let url = format!("{}/advanced/routes", cfg.api_url);
        http::post(&self.http, &cfg, &url, params).await
    }
}
