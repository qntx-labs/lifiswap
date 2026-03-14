//! `GET /tools` endpoint.

use crate::client::LiFiClient;
use crate::error::Result;
use crate::http;
use crate::types::{ToolsRequest, ToolsResponse};

impl LiFiClient {
    /// Get the available tools (bridges and exchanges).
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError`](crate::error::LiFiError) on network or API errors.
    pub async fn get_tools(&self, params: Option<&ToolsRequest>) -> Result<ToolsResponse> {
        let cfg = self.http_config();
        let mut url = format!("{}/tools", cfg.api_url);

        if let Some(p) = params
            && let Some(ref chains) = p.chains
        {
            url = format!("{url}?chains={chains}");
        }

        http::get(&self.http, &cfg, &url).await
    }
}
