//! `GET /connections` endpoint.

use crate::client::LiFiClient;
use crate::error::Result;
use crate::types::{ConnectionsRequest, ConnectionsResponse};

impl LiFiClient {
    /// Get all available connections for swap/bridging tokens.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError`](crate::error::LiFiError) on network or API errors.
    pub async fn get_connections(
        &self,
        params: &ConnectionsRequest,
    ) -> Result<ConnectionsResponse> {
        self.get("/connections", params).await
    }
}
