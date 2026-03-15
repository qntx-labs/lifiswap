//! Bitcoin error parsing.
//!
//! Maps Bitcoin-specific errors to [`LiFiError`] variants, mirroring
//! the TS SDK's `parseBitcoinErrors`.

use lifiswap::error::{LiFiError, LiFiErrorCode};

/// Parse a Bitcoin error message into an appropriate [`LiFiError`].
///
/// Categorizes common Bitcoin errors:
/// - Mempool conflicts → `TransactionConflict`
/// - Signature rejection (code 4001 / -32000 / "rejected") → `SignatureRejected`
/// - Not found (code -5 / -32700) → `NotFound`
/// - Everything else → `InternalError`
#[must_use]
pub fn parse_bitcoin_error(error: &str) -> LiFiError {
    let lower = error.to_ascii_lowercase();

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
            message: error.to_owned(),
        };
    }

    if lower.contains("not found")
        || lower.contains("-32700")
        || lower.contains("-32064")
        || lower.contains("code: -5")
    {
        return LiFiError::Transaction {
            code: LiFiErrorCode::NotFound,
            message: error.to_owned(),
        };
    }

    if lower.contains("insufficient") || lower.contains("not enough") {
        return LiFiError::Transaction {
            code: LiFiErrorCode::InsufficientFunds,
            message: error.to_owned(),
        };
    }

    LiFiError::Transaction {
        code: LiFiErrorCode::InternalError,
        message: error.to_owned(),
    }
}
