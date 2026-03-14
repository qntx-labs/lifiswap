//! `POST /advanced/stepTransaction` endpoint.

use crate::client::LiFiClient;
use crate::error::Result;
use crate::http;
use crate::types::LiFiStep;

impl LiFiClient {
    /// Get the transaction data for a single step of a route.
    ///
    /// The returned [`LiFiStep`] will have its `transaction_request` field
    /// populated with the data needed to sign and send the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError`](crate::error::LiFiError) on network or API errors.
    pub async fn get_step_transaction(&self, step: &LiFiStep) -> Result<LiFiStep> {
        let cfg = self.http_config();
        let url = format!("{}/advanced/stepTransaction", cfg.api_url);
        http::post(&self.http, &cfg, &url, step).await
    }
}
