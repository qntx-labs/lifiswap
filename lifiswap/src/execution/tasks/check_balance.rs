//! Balance check task — verifies the wallet has sufficient token balance.
//!
//! Mirrors the `TypeScript` SDK's `CheckBalanceTask` + `checkBalance` helper:
//! queries on-chain balance via the provider, retries up to 3 times on
//! insufficient balance, and auto-adjusts `fromAmount` within slippage limits.

use std::future::Future;
use std::pin::Pin;

use crate::error::{LiFiError, LiFiErrorCode, Result};
use crate::execution::task::{ExecutionContext, ExecutionTask};
use crate::types::{ExecutionActionStatus, ExecutionActionType, TaskStatus};

const BALANCE_RETRY_COUNT: u32 = 3;
const BALANCE_RETRY_DELAY_MS: u64 = 200;

/// Checks that the wallet has sufficient balance before executing a step.
///
/// Queries on-chain token balance via the provider and validates:
/// 1. Wallet address is present on the step
/// 2. On-chain balance is sufficient for `fromAmount`
/// 3. If insufficient, retries up to 3 times (200ms apart)
/// 4. If still insufficient but within slippage, adjusts `fromAmount`
/// 5. Otherwise returns a [`LiFiError::Balance`] error
#[derive(Debug, Default, Clone, Copy)]
pub struct CheckBalanceTask;

impl ExecutionTask for CheckBalanceTask {
    fn run<'a>(
        &'a self,
        ctx: &'a mut ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = Result<TaskStatus>> + Send + 'a>> {
        Box::pin(async move {
            let action_type = if ctx.is_bridge_execution {
                ExecutionActionType::CrossChain
            } else {
                ExecutionActionType::Swap
            };

            let from_chain_id = ctx.step.action.from_chain_id.0;

            ctx.status_manager.initialize_action(
                ctx.step,
                action_type,
                from_chain_id,
                ExecutionActionStatus::Started,
            )?;

            let wallet_address = ctx
                .step
                .step
                .action
                .from_address
                .as_ref()
                .ok_or_else(|| LiFiError::Transaction {
                    code: LiFiErrorCode::InternalError,
                    message: "The wallet address is undefined.".to_owned(),
                })?
                .clone();

            check_balance(ctx, &wallet_address, 0).await?;

            tracing::debug!(wallet = %wallet_address, "balance check passed");

            Ok(TaskStatus::Completed)
        })
    }
}

/// Recursively check balance with retry logic (mirrors TS `checkBalance` helper).
async fn check_balance(
    ctx: &mut ExecutionContext<'_>,
    wallet_address: &str,
    depth: u32,
) -> Result<()> {
    let from_token = ctx.step.action.from_token.clone();
    let balances = ctx
        .provider
        .get_balance(wallet_address, &[from_token])
        .await?;

    let Some(token_balance) = balances.first() else {
        return Ok(());
    };

    let current_balance: u128 = token_balance
        .amount
        .as_deref()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let needed_balance: u128 = ctx
        .step
        .step
        .action
        .from_amount
        .as_deref()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if current_balance >= needed_balance {
        return Ok(());
    }

    if depth < BALANCE_RETRY_COUNT {
        tokio::time::sleep(std::time::Duration::from_millis(BALANCE_RETRY_DELAY_MS)).await;
        return Box::pin(check_balance(ctx, wallet_address, depth + 1)).await;
    }

    let slippage = ctx.step.action.slippage.unwrap_or(0.0);
    let slippage_factor = 1.0 - slippage;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let needed_with_slippage = (needed_balance as f64 * slippage_factor) as u128;

    if needed_with_slippage <= current_balance {
        ctx.step.action.from_amount = Some(current_balance.to_string());
        tracing::info!(
            adjusted_amount = current_balance,
            "adjusted fromAmount within slippage tolerance"
        );
        return Ok(());
    }

    let symbol = &token_balance.token.symbol;
    let decimals = token_balance.token.decimals;
    let needed_fmt = format_units(needed_balance, decimals);
    let current_fmt = format_units(current_balance, decimals);

    Err(LiFiError::Balance(format!(
        "Your {symbol} balance is too low: trying to transfer {needed_fmt} {symbol}, \
         but wallet only holds {current_fmt} {symbol}. No funds have been sent."
    )))
}

fn format_units(amount: u128, decimals: u8) -> String {
    if decimals == 0 {
        return amount.to_string();
    }
    let divisor = 10u128.pow(u32::from(decimals));
    let whole = amount / divisor;
    let frac = amount % divisor;
    if frac == 0 {
        format!("{whole}")
    } else {
        let frac_str = format!("{frac:0>width$}", width = decimals as usize);
        let trimmed = frac_str.trim_end_matches('0');
        format!("{whole}.{trimmed}")
    }
}
