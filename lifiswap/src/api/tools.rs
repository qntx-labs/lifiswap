//! `GET /tools` endpoint.

use crate::client::LiFiClient;
use crate::error::Result;
use crate::types::{ToolsRequest, ToolsResponse};

impl LiFiClient {
    /// Get the available tools (bridges and exchanges).
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError`](crate::error::LiFiError) on network or API errors.
    pub async fn get_tools(&self, params: Option<&ToolsRequest>) -> Result<ToolsResponse> {
        match params {
            Some(p) => self.get("/tools", p).await,
            None => self.get("/tools", &()).await,
        }
    }
}
