//! `POST /advanced/routes` endpoint.

use crate::client::LiFiClient;
use crate::error::Result;
use crate::types::{RoutesRequest, RoutesResponse};

impl LiFiClient {
    /// Get available routes for a token transfer.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError`](crate::error::LiFiError) on network or API errors.
    pub async fn get_routes(&self, params: &RoutesRequest) -> Result<RoutesResponse> {
        self.post("/advanced/routes", params).await
    }
}
