//! `POST /advanced/stepTransaction` endpoint.

use crate::client::LiFiClient;
use crate::error::Result;
use crate::types::{LiFiStep, SignedLiFiStep};

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

    /// Get the transaction data for a step, including signed typed data.
    ///
    /// Use this variant when Permit2 or native EIP-2612 permit signatures
    /// have been collected and need to be forwarded to the API so it can
    /// embed them in the final transaction calldata.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError`](crate::error::LiFiError) on network or API errors.
    pub async fn get_step_transaction_with_signatures(
        &self,
        step: &SignedLiFiStep,
    ) -> Result<LiFiStep> {
        self.post("/advanced/stepTransaction", step).await
    }
}
