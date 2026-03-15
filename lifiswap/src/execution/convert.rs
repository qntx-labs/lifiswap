//! Convert a quote (`LiFiStep`) into a [`Route`] for execution.

use crate::error::{LiFiError, Result};
use crate::types::{Insurance, LiFiStep, Route, RouteBase};

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
///
/// # Example
///
/// ```ignore
/// let route = convert_quote_to_route(&quote_step, None)?;
/// let extended = client.execute_route(route, Default::default()).await?;
/// ```
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
        && let Some(ref prev_est) = prev.estimate
    {
        to_amount = prev_est.to_amount.clone().unwrap_or_default();
        to_amount_min.clone_from(&prev_est.to_amount_min);
        to_amount_usd.clone_from(&prev_est.to_amount_usd);
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
        base: RouteBase {
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
            tags: None,
            insurance: Some(Insurance {
                state: "NOT_INSURABLE".to_owned(),
                fee_amount_usd: Some("0".to_owned()),
            }),
            gas_cost_usd,
        },
        steps: vec![quote.clone()],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Action, ChainId, Estimate, StepType, Token};

    fn test_token(chain_id: u64) -> Token {
        Token {
            address: "0xA0b8...".to_owned(),
            decimals: 6,
            symbol: "USDC".to_owned(),
            chain_id: ChainId(chain_id),
            coin_key: None,
            name: "USD Coin".to_owned(),
            logo_uri: None,
            price_usd: Some("1.0".to_owned()),
        }
    }

    fn test_quote() -> LiFiStep {
        LiFiStep {
            id: "step-1".to_owned(),
            step_type: StepType::Swap,
            tool: Some("uniswap".to_owned()),
            tool_details: None,
            action: Action {
                from_chain_id: ChainId(1),
                to_chain_id: ChainId(137),
                from_token: test_token(1),
                to_token: test_token(137),
                from_amount: Some("1000000".to_owned()),
                from_address: Some("0xSender".to_owned()),
                to_address: Some("0xReceiver".to_owned()),
                slippage: Some(0.03),
                destination_call_data: None,
            },
            estimate: Some(Estimate {
                tool: Some("uniswap".to_owned()),
                from_amount: Some("1000000".to_owned()),
                from_amount_usd: Some("1.00".to_owned()),
                to_amount: Some("990000".to_owned()),
                to_amount_min: Some("960000".to_owned()),
                to_amount_usd: Some("0.99".to_owned()),
                approval_address: None,
                approval_reset: None,
                execution_duration: Some(30.0),
                fee_costs: None,
                gas_costs: None,
            }),
            included_steps: None,
            integrator: None,
            transaction_request: None,
            execution: None,
            typed_data: None,
            insurance: None,
        }
    }

    #[test]
    fn converts_quote_to_route() {
        let quote = test_quote();
        let route = convert_quote_to_route(&quote, None).expect("should convert");

        assert_eq!(route.id, "step-1");
        assert_eq!(route.from_chain_id, ChainId(1));
        assert_eq!(route.to_chain_id, ChainId(137));
        assert_eq!(route.from_amount, "1000000");
        assert_eq!(route.to_amount, "990000");
        assert_eq!(route.from_amount_usd.as_deref(), Some("1.00"));
        assert_eq!(route.to_amount_usd.as_deref(), Some("0.99"));
        assert_eq!(route.to_amount_min.as_deref(), Some("960000"));
        assert_eq!(route.from_address.as_deref(), Some("0xSender"));
        assert_eq!(route.to_address.as_deref(), Some("0xReceiver"));
        assert_eq!(route.steps.len(), 1);
        assert!(route.insurance.is_some());
    }

    #[test]
    fn uses_from_address_when_to_address_missing() {
        let mut quote = test_quote();
        quote.action.to_address = None;

        let route = convert_quote_to_route(&quote, None).expect("should convert");
        assert_eq!(route.to_address.as_deref(), Some("0xSender"));
    }

    #[test]
    fn errors_without_estimate() {
        let mut quote = test_quote();
        quote.estimate = None;

        let err = convert_quote_to_route(&quote, None).unwrap_err();
        assert!(err.to_string().contains("no estimate"), "{err}");
    }

    #[test]
    fn errors_without_from_amount_usd() {
        let mut quote = test_quote();
        quote
            .estimate
            .as_mut()
            .expect("has estimate")
            .from_amount_usd = None;

        let err = convert_quote_to_route(&quote, None).unwrap_err();
        assert!(err.to_string().contains("from_amount_usd"), "{err}");
    }
}
