//! `POST /advanced/stepTransaction` endpoint.

use crate::client::LiFiClient;
use crate::error::Result;
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
        self.post("/advanced/stepTransaction", step).await
    }
}
