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
        let mut url = url::Url::parse(&format!(
            "{}/gas/suggestion/{}",
            self.api_url(),
            params.chain_id
        ))?;

        {
            let mut qs = url.query_pairs_mut();
            if let Some(ref fc) = params.from_chain {
                qs.append_pair("fromChain", &fc.to_string());
            }
            if let Some(ref ft) = params.from_token {
                qs.append_pair("fromToken", ft);
            }
        }

        self.get_url(&url).await
    }
}
