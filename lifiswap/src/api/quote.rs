//! `GET /quote` and `POST /quote/contractCalls` endpoints.

use crate::client::LiFiClient;
use crate::error::{LiFiError, Result};
use crate::http;
use crate::types::{ContractCallsQuoteRequest, LiFiStep, QuoteRequest, QuoteToAmountRequest};

impl LiFiClient {
    /// Get a quote for a token transfer using `fromAmount`.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError::Validation`] if required fields are missing, or
    /// [`LiFiError`] on network/API errors.
    pub async fn get_quote(&self, params: &QuoteRequest) -> Result<LiFiStep> {
        if params.from_chain.is_empty() {
            return Err(LiFiError::Validation(
                "required parameter \"fromChain\" is missing".into(),
            ));
        }
        if params.from_token.is_empty() {
            return Err(LiFiError::Validation(
                "required parameter \"fromToken\" is missing".into(),
            ));
        }
        if params.from_address.is_empty() {
            return Err(LiFiError::Validation(
                "required parameter \"fromAddress\" is missing".into(),
            ));
        }
        if params.from_amount.is_empty() {
            return Err(LiFiError::Validation(
                "required parameter \"fromAmount\" is missing".into(),
            ));
        }

        let cfg = self.http_config();
        let mut query = vec![
            ("fromChain", params.from_chain.as_str()),
            ("fromToken", params.from_token.as_str()),
            ("fromAddress", params.from_address.as_str()),
            ("fromAmount", params.from_amount.as_str()),
            ("toChain", params.to_chain.as_str()),
            ("toToken", params.to_token.as_str()),
        ];

        let integrator_val = params.integrator.as_deref().unwrap_or(&cfg.integrator);
        query.push(("integrator", integrator_val));

        if let Some(ref addr) = params.to_address {
            query.push(("toAddress", addr.as_str()));
        }

        let slippage_str;
        if let Some(s) = params.slippage {
            slippage_str = s.to_string();
            query.push(("slippage", &slippage_str));
        }

        let fee_str;
        if let Some(f) = params.fee {
            fee_str = f.to_string();
            query.push(("fee", &fee_str));
        }

        if let Some(ref r) = params.referrer {
            query.push(("referrer", r.as_str()));
        }

        let base = url::Url::parse(&format!("{}/quote", cfg.api_url))?;
        let url = url::Url::parse_with_params(base.as_str(), &query)?;

        http::get(&self.http, &cfg, url.as_str()).await
    }

    /// Get a quote for a token transfer using `toAmount` (reverse quote).
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError`] on validation, network, or API errors.
    pub async fn get_quote_to_amount(&self, params: &QuoteToAmountRequest) -> Result<LiFiStep> {
        let cfg = self.http_config();
        let integrator_val = params.integrator.as_deref().unwrap_or(&cfg.integrator);

        let mut query = vec![
            ("fromChain", params.from_chain.as_str()),
            ("fromToken", params.from_token.as_str()),
            ("fromAddress", params.from_address.as_str()),
            ("toAmount", params.to_amount.as_str()),
            ("toChain", params.to_chain.as_str()),
            ("toToken", params.to_token.as_str()),
            ("integrator", integrator_val),
        ];

        let slippage_str;
        if let Some(s) = params.slippage {
            slippage_str = s.to_string();
            query.push(("slippage", &slippage_str));
        }

        let base = url::Url::parse(&format!("{}/quote/toAmount", cfg.api_url))?;
        let url = url::Url::parse_with_params(base.as_str(), &query)?;

        http::get(&self.http, &cfg, url.as_str()).await
    }

    /// Get a quote for a destination contract call.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError`] on validation, network, or API errors.
    pub async fn get_contract_calls_quote(
        &self,
        params: &ContractCallsQuoteRequest,
    ) -> Result<LiFiStep> {
        if params.from_chain.is_empty() {
            return Err(LiFiError::Validation(
                "required parameter \"fromChain\" is missing".into(),
            ));
        }
        if params.contract_calls.is_empty() {
            return Err(LiFiError::Validation(
                "required parameter \"contractCalls\" is missing".into(),
            ));
        }

        let cfg = self.http_config();
        let url = format!("{}/quote/contractCalls", cfg.api_url);
        http::post(&self.http, &cfg, &url, params).await
    }
}
