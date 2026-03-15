//! Solana error parsing — transforms low-level Solana errors into
//! user-friendly [`LiFiError`] variants.
//!
//! Mirrors the TS SDK's `parseSolanaErrors.ts`.

use lifiswap::error::{LiFiError, LiFiErrorCode};

/// Classify a Solana error into an appropriate [`LiFiError`].
///
/// Called by [`SvmStepExecutor`](crate::executor::SvmStepExecutor) to wrap
/// errors before propagating them to the execution engine.
pub fn parse_solana_error(error: LiFiError) -> LiFiError {
    let msg = error.to_string();
    let lower = msg.to_ascii_lowercase();

    if is_signature_rejected(&lower) {
        return LiFiError::Transaction {
            code: LiFiErrorCode::SignatureRejected,
            message: msg,
        };
    }

    if lower.contains("blockhash not found")
        || lower.contains("block height exceeded")
        || lower.contains("transaction has expired")
    {
        return LiFiError::Transaction {
            code: LiFiErrorCode::TransactionExpired,
            message: msg,
        };
    }

    if lower.contains("simulation failed") || lower.contains("simulate") {
        return LiFiError::Transaction {
            code: LiFiErrorCode::TransactionSimulationFailed,
            message: msg,
        };
    }

    if lower.contains("insufficient funds") || lower.contains("insufficient lamports") {
        return LiFiError::Transaction {
            code: LiFiErrorCode::InsufficientFunds,
            message: msg,
        };
    }

    if lower.contains("sendtransactionerror") {
        return LiFiError::Transaction {
            code: LiFiErrorCode::TransactionFailed,
            message: msg,
        };
    }

    error
}

fn is_signature_rejected(msg: &str) -> bool {
    msg.contains("user rejected")
        || msg.contains("user denied")
        || msg.contains("user cancelled")
        || msg.contains("user canceled")
        || msg.contains("walletsigntransactionerror")
}
