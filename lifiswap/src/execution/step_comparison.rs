//! Step comparison logic for exchange rate change detection.
//!
//! Mirrors the TypeScript SDK's `stepComparison.ts` and `checkStepSlippageThreshold`.

use crate::error::{LiFiError, LiFiErrorCode, Result};
use crate::execution::status::StatusManager;
use crate::types::{ExchangeRateUpdateParams, LiFiStep};

const STANDARD_THRESHOLD: f64 = 0.005;

/// Check whether the exchange rate difference between old and new step
/// estimates is within the slippage threshold.
///
/// Returns `true` if the rate change is acceptable.
#[must_use]
pub fn check_step_slippage_threshold(old_step: &LiFiStep, new_step: &LiFiStep) -> bool {
    let set_slippage = old_step.action.slippage.unwrap_or(STANDARD_THRESHOLD);

    let old_to_amount_min: u128 = old_step
        .estimate
        .as_ref()
        .and_then(|e| e.to_amount_min.as_deref())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let new_to_amount_min: u128 = new_step
        .estimate
        .as_ref()
        .and_then(|e| e.to_amount_min.as_deref())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if old_to_amount_min == 0 {
        return true;
    }

    let difference = old_to_amount_min.saturating_sub(new_to_amount_min);

    #[allow(clippy::cast_precision_loss)]
    let actual_slippage =
        (difference as f64 * 1_000_000_000.0 / old_to_amount_min as f64) / 1_000_000_000.0;

    actual_slippage <= set_slippage
}

/// Compare old and new step data, invoking the exchange rate update hook
/// if the rate changed beyond the slippage threshold.
///
/// Returns the updated step if accepted, or an error if rejected.
pub async fn step_comparison(
    status_manager: &StatusManager,
    old_step: &LiFiStep,
    new_step: LiFiStep,
    allow_user_interaction: bool,
    accept_hook: Option<crate::types::AcceptExchangeRateUpdateHook>,
) -> Result<LiFiStep> {
    if check_step_slippage_threshold(old_step, &new_step) {
        return Ok(new_step);
    }

    let mut allow_step_update = false;
    if allow_user_interaction {
        if let Some(hook) = accept_hook {
            let old_to_amount = old_step
                .estimate
                .as_ref()
                .and_then(|e| e.to_amount.clone())
                .unwrap_or_default();
            let new_to_amount = new_step
                .estimate
                .as_ref()
                .and_then(|e| e.to_amount.clone())
                .unwrap_or_default();

            let params = ExchangeRateUpdateParams {
                to_token: new_step.action.to_token.clone(),
                old_to_amount,
                new_to_amount,
            };
            allow_step_update = hook(params).await;
        }
    }

    if !allow_step_update {
        return Err(LiFiError::Transaction {
            code: LiFiErrorCode::ExchangeRateUpdateCanceled,
            message: "Exchange rate has changed!\n\
                Transaction was not sent, your funds are still in your wallet.\n\
                The exchange rate has changed and the previous estimation \
                can not be fulfilled due to value loss."
                .to_owned(),
        });
    }

    let _ = status_manager;
    Ok(new_step)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Action, ChainId, Estimate, Token};

    fn make_token() -> Token {
        Token {
            address: "0x0".to_owned(),
            decimals: 18,
            symbol: "TST".to_owned(),
            chain_id: ChainId(1),
            coin_key: None,
            name: "Test".to_owned(),
            logo_uri: None,
            price_usd: None,
        }
    }

    fn make_step(slippage: f64, to_amount_min: &str) -> LiFiStep {
        LiFiStep {
            id: "s1".to_owned(),
            step_type: "swap".to_owned(),
            tool: None,
            tool_details: None,
            action: Action {
                from_chain_id: ChainId(1),
                to_chain_id: ChainId(1),
                from_token: make_token(),
                to_token: make_token(),
                from_amount: None,
                from_address: None,
                to_address: None,
                slippage: Some(slippage),
                destination_call_data: None,
            },
            estimate: Some(Estimate {
                tool: None,
                from_amount: None,
                to_amount: Some("1000000".to_owned()),
                to_amount_min: Some(to_amount_min.to_owned()),
                approval_address: None,
                fee_costs: None,
                gas_costs: None,
                execution_duration: None,
                from_amount_usd: None,
                to_amount_usd: None,
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
    fn within_threshold() {
        let old = make_step(0.03, "1000000");
        let new = make_step(0.03, "975000");
        assert!(check_step_slippage_threshold(&old, &new));
    }

    #[test]
    fn exceeds_threshold() {
        let old = make_step(0.01, "1000000");
        let new = make_step(0.01, "980000");
        assert!(!check_step_slippage_threshold(&old, &new));
    }

    #[test]
    fn zero_old_amount() {
        let old = make_step(0.03, "0");
        let new = make_step(0.03, "500000");
        assert!(check_step_slippage_threshold(&old, &new));
    }
}
