//! `POST /patcher` endpoint.

use crate::client::LiFiClient;
use crate::error::{LiFiError, Result};
use crate::types::{PatchCallDataEntry, PatchContractCallsResponse};

impl LiFiClient {
    /// Patch contract call data for cross-chain operations.
    ///
    /// The API accepts an array of call data entries and returns
    /// the patched contract calls with updated amounts.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError::Validation`] if `entries` is empty, or
    /// [`LiFiError`] on network/API errors.
    pub async fn patch_contract_calls(
        &self,
        entries: &[PatchCallDataEntry],
    ) -> Result<Vec<PatchContractCallsResponse>> {
        if entries.is_empty() {
            return Err(LiFiError::Validation(
                "at least one patch entry is required".into(),
            ));
        }

        self.post("/patcher", entries).await
    }
}
