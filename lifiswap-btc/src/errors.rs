//! Bitcoin error parsing.
//!
//! Maps Bitcoin-specific errors to [`LiFiError`] variants, mirroring
//! the TS SDK's `parseBitcoinErrors`.

use lifiswap::error::{LiFiError, LiFiErrorCode};

/// Classify a Bitcoin error into an appropriate [`LiFiError`].
///
/// Called by the step executor to wrap errors before propagating
/// them to the execution engine.
pub fn parse_bitcoin_error(error: LiFiError) -> LiFiError {
    let msg = error.to_string();
    let lower = msg.to_ascii_lowercase();

    if lower.contains("conflict") {
        return LiFiError::Transaction {
            code: LiFiErrorCode::TransactionConflict,
            message: "Transaction conflicts with another transaction in the mempool.".to_owned(),
        };
    }

    if lower.contains("rejected")
        || lower.contains("4001")
        || lower.contains("-32000")
        || lower.contains("user denied")
        || lower.contains("user cancel")
    {
        return LiFiError::Transaction {
            code: LiFiErrorCode::SignatureRejected,
            message: msg,
        };
    }

    if lower.contains("not found")
        || lower.contains("-32700")
        || lower.contains("-32064")
        || lower.contains("code: -5")
    {
        return LiFiError::Transaction {
            code: LiFiErrorCode::NotFound,
            message: msg,
        };
    }

    if lower.contains("insufficient") || lower.contains("not enough") {
        return LiFiError::Transaction {
            code: LiFiErrorCode::InsufficientFunds,
            message: msg,
        };
    }

    error
}
