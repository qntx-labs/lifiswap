//! `GET /gas/suggestion/{chainId}` endpoint.

use crate::client::LiFiClient;
use crate::error::Result;
use crate::types::{GasRecommendationRequest, GasRecommendationResponse};

impl LiFiClient {
    /// Get gas recommendation for a chain.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError::Validation`] if `chain_id` is missing, or
    /// [`LiFiError`] on network/API errors.
    pub async fn get_gas_recommendation(
        &self,
        params: &GasRecommendationRequest,
    ) -> Result<GasRecommendationResponse> {
        let path = format!("/gas/suggestion/{}", params.chain_id);
        self.get(&path, params).await
    }
}
