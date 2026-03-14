//! Convert a quote (`LiFiStep`) into a [`Route`] for execution.

use crate::error::{LiFiError, Result};
use crate::types::{Insurance, LiFiStep, Route};

/// Options for controlling quote-to-route conversion behavior.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConvertQuoteOptions {
    /// When `true`, if the quote has zero output values (`to_amount`, `to_amount_min`),
    /// use values from the last included step that has non-zero output.
    pub adjust_zero_output_from_previous_step: bool,
}

fn parse_bigint(value: Option<&str>) -> u128 {
    value.and_then(|v| v.parse::<u128>().ok()).unwrap_or(0)
}

fn parse_number(value: Option<&str>) -> f64 {
    value.and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0)
}

fn is_zero_output(
    to_amount: Option<&str>,
    to_amount_min: Option<&str>,
    to_amount_usd: Option<&str>,
) -> bool {
    parse_bigint(to_amount) == 0
        && parse_bigint(to_amount_min) == 0
        && parse_number(to_amount_usd) == 0.0
}

/// Convert a quote ([`LiFiStep`]) into a [`Route`].
///
/// This is useful when you have a single-step quote from `get_quote` and
/// need to convert it into a `Route` for the execution engine.
///
/// # Errors
///
/// Returns [`LiFiError::Validation`] if the step estimate is missing
/// required USD amount fields.
pub fn convert_quote_to_route(
    quote: &LiFiStep,
    options: Option<ConvertQuoteOptions>,
) -> Result<Route> {
    let estimate = quote
        .estimate
        .as_ref()
        .ok_or_else(|| LiFiError::Validation("Quote has no estimate.".to_owned()))?;

    let mut to_amount = estimate.to_amount.clone().unwrap_or_default();
    let mut to_amount_min = estimate.to_amount_min.clone();
    let mut to_amount_usd = estimate.to_amount_usd.clone();

    let opts = options.unwrap_or_default();
    if opts.adjust_zero_output_from_previous_step
        && let Some(ref included) = quote.included_steps
            && !included.is_empty()
                && is_zero_output(
                    Some(&to_amount),
                    to_amount_min.as_deref(),
                    to_amount_usd.as_deref(),
                )
                && let Some(prev) = included.iter().rev().find(|s| {
                    s.estimate.as_ref().is_some_and(|e| {
                        parse_bigint(e.to_amount.as_deref()) > 0
                            || parse_bigint(e.to_amount_min.as_deref()) > 0
                    })
                })
                    && let Some(ref prev_est) = prev.estimate {
                        to_amount = prev_est.to_amount.clone().unwrap_or_default();
                        to_amount_min = prev_est.to_amount_min.clone();
                        to_amount_usd = prev_est.to_amount_usd.clone();
                    }

    let from_amount_usd = estimate
        .from_amount_usd
        .as_ref()
        .ok_or_else(|| {
            LiFiError::Validation("Missing 'from_amount_usd' in step estimate.".to_owned())
        })?
        .clone();

    let gas_cost_usd = estimate
        .gas_costs
        .as_ref()
        .and_then(|costs| costs.first())
        .and_then(|c| c.amount_usd.clone());

    Ok(Route {
        id: quote.id.clone(),
        from_chain_id: quote.action.from_chain_id,
        to_chain_id: quote.action.to_chain_id,
        from_token: quote.action.from_token.clone(),
        to_token: quote.action.to_token.clone(),
        from_amount: quote.action.from_amount.clone().unwrap_or_default(),
        to_amount,
        from_amount_usd: Some(from_amount_usd),
        to_amount_usd,
        to_amount_min,
        from_address: quote.action.from_address.clone(),
        to_address: quote
            .action
            .to_address
            .clone()
            .or_else(|| quote.action.from_address.clone()),
        steps: vec![quote.clone()],
        tags: None,
        insurance: Some(Insurance {
            state: "NOT_INSURABLE".to_owned(),
            fee_amount_usd: Some("0".to_owned()),
        }),
        gas_cost_usd,
    })
}
