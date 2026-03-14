//! `GET /status` endpoint.

use crate::client::LiFiClient;
use crate::error::{LiFiError, Result};
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

        self.get("/status", params).await
    }
}
