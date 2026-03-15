//! Relay endpoints (`POST /advanced/relay`, `GET /relayer/quote`, `GET /relayer/status`).

use super::quote::{QuoteRouteFields, push_route_option_params};
use crate::client::LiFiClient;
use crate::error::{LiFiError, Result};
use crate::types::{
    LiFiStep, QuoteRequest, RelayRequest, RelayResponse, RelayResponseData, RelayStatusRequest,
    RelayStatusResponse, RelayStatusResponseData, TransactionAnalyticsRequest,
    TransactionAnalyticsResponse,
};

impl LiFiClient {
    /// Get a relayer quote for a gasless token transfer.
    ///
    /// Fields not set on the request are filled from
    /// [`LiFiConfig::route_options`](crate::client::LiFiConfig::route_options) if configured.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError`] on network or API errors.
    pub async fn get_relayer_quote(&self, params: &QuoteRequest) -> Result<LiFiStep> {
        let defaults = self.inner.config.route_options.as_ref();
        let integrator = &self.inner.config.integrator;

        let mut query: Vec<(String, String)> = vec![
            ("fromChain".into(), params.from_chain.clone()),
            ("fromToken".into(), params.from_token.clone()),
            ("fromAddress".into(), params.from_address.clone()),
            ("fromAmount".into(), params.from_amount.clone()),
            ("toChain".into(), params.to_chain.clone()),
            ("toToken".into(), params.to_token.clone()),
            (
                "integrator".into(),
                params
                    .integrator
                    .clone()
                    .unwrap_or_else(|| integrator.clone()),
            ),
        ];

        push_route_option_params(
            &mut query,
            &QuoteRouteFields::resolve_basic(
                params.order,
                params.slippage,
                params.fee,
                params.referrer.as_deref(),
                defaults,
            ),
        );

        let base = url::Url::parse(&format!("{}/relayer/quote", self.api_url()))?;
        let url = url::Url::parse_with_params(base.as_str(), &query)?;

        let resp: serde_json::Value = self.get_url(&url).await?;

        if resp.get("status").and_then(serde_json::Value::as_str) == Some("error") {
            let code = resp
                .pointer("/data/code")
                .and_then(serde_json::Value::as_i64)
                .and_then(|v| i32::try_from(v).ok())
                .unwrap_or(0);
            let message = resp
                .pointer("/data/message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown error")
                .to_owned();
            return Err(LiFiError::Server { code, message });
        }

        let step: LiFiStep = serde_json::from_value(resp.get("data").cloned().unwrap_or(resp))?;
        Ok(step)
    }

    /// Relay a signed transaction through the relayer service.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError::Validation`] if `typed_data` is empty, or
    /// [`LiFiError`] on network/API errors.
    pub async fn relay_transaction(&self, params: &RelayRequest) -> Result<RelayResponseData> {
        if params.typed_data.is_empty() {
            return Err(LiFiError::Validation(
                "required parameter \"typedData\" is missing".into(),
            ));
        }

        let resp: RelayResponse = self.post("/advanced/relay", params).await?;

        if resp.status == "error" {
            return Err(LiFiError::Server {
                code: resp.data.code.unwrap_or(0),
                message: resp.data.message.unwrap_or_default(),
            });
        }

        Ok(resp.data)
    }

    /// Check the status of a relayed transaction.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError`] on network or API errors.
    pub async fn get_relayed_transaction_status(
        &self,
        params: &RelayStatusRequest,
    ) -> Result<RelayStatusResponseData> {
        let resp: RelayStatusResponse = self
            .get("/relayer/status", &[("taskId", params.task_id.as_str())])
            .await?;
        Ok(resp.data)
    }

    /// Get transaction history (analytics) for a wallet.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError`] on network or API errors.
    pub async fn get_transaction_history(
        &self,
        params: &TransactionAnalyticsRequest,
    ) -> Result<TransactionAnalyticsResponse> {
        self.get("/analytics/transfers", params).await
    }
}
