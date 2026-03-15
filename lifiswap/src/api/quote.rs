//! `GET /quote` and `POST /quote/contractCalls` endpoints.

use crate::client::LiFiClient;
use crate::error::{LiFiError, Result};
use crate::types::{
    ContractCallsQuoteRequest, LiFiStep, QuoteRequest, QuoteToAmountRequest, RouteOptions,
};

impl LiFiClient {
    /// Get a quote for a token transfer using `fromAmount`.
    ///
    /// Fields not set on the request are filled from
    /// [`LiFiConfig::route_options`](crate::client::LiFiConfig::route_options) if configured.
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

        if let Some(ref addr) = params.to_address {
            query.push(("toAddress".into(), addr.clone()));
        }

        push_route_option_params(&mut query, &QuoteRouteFields::resolve(params, defaults));

        let base = url::Url::parse(&format!("{}/quote", self.api_url()))?;
        let url = url::Url::parse_with_params(base.as_str(), &query)?;

        self.get_url(&url).await
    }

    /// Get a quote for a token transfer using `toAmount` (reverse quote).
    ///
    /// Fields not set on the request are filled from
    /// [`LiFiConfig::route_options`](crate::client::LiFiConfig::route_options) if configured.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError`] on validation, network, or API errors.
    pub async fn get_quote_to_amount(&self, params: &QuoteToAmountRequest) -> Result<LiFiStep> {
        let defaults = self.inner.config.route_options.as_ref();
        let integrator = &self.inner.config.integrator;

        let mut query: Vec<(String, String)> = vec![
            ("fromChain".into(), params.from_chain.clone()),
            ("fromToken".into(), params.from_token.clone()),
            ("fromAddress".into(), params.from_address.clone()),
            ("toAmount".into(), params.to_amount.clone()),
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

        if let Some(ref addr) = params.to_address {
            query.push(("toAddress".into(), addr.clone()));
        }

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

        let base = url::Url::parse(&format!("{}/quote/toAmount", self.api_url()))?;
        let url = url::Url::parse_with_params(base.as_str(), &query)?;

        self.get_url(&url).await
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

        self.post("/quote/contractCalls", params).await
    }
}

/// Push merged route option query params onto `query`.
pub(super) fn push_route_option_params(
    query: &mut Vec<(String, String)>,
    fields: &QuoteRouteFields,
) {
    if let Some(o) = fields.order
        && let Ok(v) = serde_json::to_value(o)
        && let Some(s) = v.as_str()
    {
        query.push(("order".into(), s.to_owned()));
    }
    if let Some(s) = fields.slippage {
        query.push(("slippage".into(), s.to_string()));
    }
    if let Some(f) = fields.fee {
        query.push(("fee".into(), f.to_string()));
    }
    if let Some(ref r) = fields.referrer {
        query.push(("referrer".into(), r.clone()));
    }
    if let Some(ref v) = fields.allow_bridges {
        query.push(("allowBridges".into(), v.join(",")));
    }
    if let Some(ref v) = fields.deny_bridges {
        query.push(("denyBridges".into(), v.join(",")));
    }
    if let Some(ref v) = fields.prefer_bridges {
        query.push(("preferBridges".into(), v.join(",")));
    }
    if let Some(ref v) = fields.allow_exchanges {
        query.push(("allowExchanges".into(), v.join(",")));
    }
    if let Some(ref v) = fields.deny_exchanges {
        query.push(("denyExchanges".into(), v.join(",")));
    }
    if let Some(ref v) = fields.prefer_exchanges {
        query.push(("preferExchanges".into(), v.join(",")));
    }
}

/// Resolved route option fields after merging request-level values with config defaults.
/// Owns all data to avoid cross-lifetime borrowing issues.
#[derive(Default)]
pub(super) struct QuoteRouteFields {
    order: Option<crate::types::Order>,
    slippage: Option<f64>,
    fee: Option<f64>,
    referrer: Option<String>,
    allow_bridges: Option<Vec<String>>,
    deny_bridges: Option<Vec<String>>,
    prefer_bridges: Option<Vec<String>>,
    allow_exchanges: Option<Vec<String>>,
    deny_exchanges: Option<Vec<String>>,
    prefer_exchanges: Option<Vec<String>>,
}

impl QuoteRouteFields {
    /// Resolve all route option fields from a [`QuoteRequest`] + config defaults.
    pub(super) fn resolve(params: &QuoteRequest, defaults: Option<&RouteOptions>) -> Self {
        let d = defaults.cloned().unwrap_or_default();
        Self {
            order: params.order.or(d.order),
            slippage: params.slippage.or(d.slippage),
            fee: params.fee.or(d.fee),
            referrer: params.referrer.clone().or(d.referrer),
            allow_bridges: params
                .allow_bridges
                .clone()
                .or_else(|| d.bridges.as_ref().and_then(|b| b.allow.clone())),
            deny_bridges: params
                .deny_bridges
                .clone()
                .or_else(|| d.bridges.as_ref().and_then(|b| b.deny.clone())),
            prefer_bridges: params
                .prefer_bridges
                .clone()
                .or_else(|| d.bridges.as_ref().and_then(|b| b.prefer.clone())),
            allow_exchanges: params
                .allow_exchanges
                .clone()
                .or_else(|| d.exchanges.as_ref().and_then(|b| b.allow.clone())),
            deny_exchanges: params
                .deny_exchanges
                .clone()
                .or_else(|| d.exchanges.as_ref().and_then(|b| b.deny.clone())),
            prefer_exchanges: params
                .prefer_exchanges
                .clone()
                .or_else(|| d.exchanges.as_ref().and_then(|b| b.prefer.clone())),
        }
    }

    /// Resolve basic fields (order, slippage, fee, referrer) only.
    pub(super) fn resolve_basic(
        order: Option<crate::types::Order>,
        slippage: Option<f64>,
        fee: Option<f64>,
        referrer: Option<&str>,
        defaults: Option<&RouteOptions>,
    ) -> Self {
        let d = defaults.cloned().unwrap_or_default();
        Self {
            order: order.or(d.order),
            slippage: slippage.or(d.slippage),
            fee: fee.or(d.fee),
            referrer: referrer.map(String::from).or(d.referrer),
            ..Self::default()
        }
    }
}
