//! `POST /advanced/routes` endpoint.

use crate::client::LiFiClient;
use crate::error::Result;
use crate::types::{RouteOptions, RoutesRequest, RoutesResponse};

impl LiFiClient {
    /// Get available routes for a token transfer.
    ///
    /// If [`LiFiConfig::route_options`](crate::client::LiFiConfig::route_options) is configured,
    /// missing fields in `params.options` are filled from those defaults.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError`](crate::error::LiFiError) on network or API errors.
    pub async fn get_routes(&self, params: &RoutesRequest) -> Result<RoutesResponse> {
        if let Some(defaults) = self.inner.config.route_options.as_ref() {
            let merged = merge_route_options(params.options.as_ref(), defaults);
            let mut body = params.clone();
            body.options = Some(merged);
            self.post("/advanced/routes", &body).await
        } else {
            self.post("/advanced/routes", params).await
        }
    }
}

/// Merge request-level route options with config defaults.
/// Request values take precedence over defaults.
fn merge_route_options(req: Option<&RouteOptions>, defaults: &RouteOptions) -> RouteOptions {
    req.map_or_else(
        || defaults.clone(),
        |r| RouteOptions {
            order: r.order.or(defaults.order),
            slippage: r.slippage.or(defaults.slippage),
            max_price_impact: r.max_price_impact.or(defaults.max_price_impact),
            fee: r.fee.or(defaults.fee),
            referrer: r.referrer.clone().or_else(|| defaults.referrer.clone()),
            bridges: r.bridges.clone().or_else(|| defaults.bridges.clone()),
            exchanges: r.exchanges.clone().or_else(|| defaults.exchanges.clone()),
            allow_switch_chain: r.allow_switch_chain.or(defaults.allow_switch_chain),
            jito_bundle: r.jito_bundle.or(defaults.jito_bundle),
            svm_sponsor: r
                .svm_sponsor
                .clone()
                .or_else(|| defaults.svm_sponsor.clone()),
        },
    )
}
