//! EVM error parsing — transforms low-level alloy/RPC errors into
//! user-friendly [`LiFiError`] variants.
//!
//! Mirrors the TS SDK's `parseEthereumErrors.ts`.

use lifiswap::error::{LiFiError, LiFiErrorCode};

/// Classify a stringified EVM/RPC error into an appropriate [`LiFiError`].
///
/// This is called by [`EvmStepExecutor`](crate::executor::EvmStepExecutor)
/// to wrap errors from the signer and on-chain interactions before
/// propagating them to the execution engine.
pub(crate) fn parse_evm_error(error: LiFiError) -> LiFiError {
    let msg = error.to_string();
    let lower = msg.to_ascii_lowercase();

    // User rejected the signature / transaction request
    if is_user_rejection(&lower) {
        return LiFiError::Transaction {
            code: LiFiErrorCode::SignatureRejected,
            message: msg,
        };
    }

    // Insufficient funds for gas + value
    if lower.contains("insufficient funds")
        || lower.contains("insufficient balance")
        || lower.contains("insufficientprefund")
    {
        return LiFiError::Transaction {
            code: LiFiErrorCode::InsufficientFunds,
            message: msg,
        };
    }

    // Out of gas / gas limit too low
    if lower.contains("out of gas") || lower.contains("gas limit") {
        return LiFiError::Transaction {
            code: LiFiErrorCode::GasLimitError,
            message: msg,
        };
    }

    // Transaction underpriced (nonce conflicts, replacement issues)
    if lower.contains("underpriced") || lower.contains("replacement transaction") {
        return LiFiError::Transaction {
            code: LiFiErrorCode::TransactionUnderpriced,
            message: msg,
        };
    }

    // Nonce too low / already known
    if lower.contains("nonce too low") || lower.contains("already known") {
        return LiFiError::Transaction {
            code: LiFiErrorCode::TransactionConflict,
            message: msg,
        };
    }

    // Internal JSON-RPC error (-32603)
    if lower.contains("-32603") {
        return LiFiError::Transaction {
            code: LiFiErrorCode::TransactionRejected,
            message: msg,
        };
    }

    // EIP-7702 upgrade rejection → StepRetry
    if is_7702_upgrade_rejection(&lower) {
        return LiFiError::StepRetry {
            message: "Wallet rejected EIP-7702 upgrade; retrying without atomicity".to_owned(),
            retry_params: [(
                "atomicityNotReady".to_owned(),
                serde_json::Value::Bool(true),
            )]
            .into_iter()
            .collect(),
        };
    }

    error
}

fn is_user_rejection(msg: &str) -> bool {
    msg.contains("user rejected")
        || msg.contains("user denied")
        || msg.contains("user cancelled")
        || msg.contains("user canceled")
        || msg.contains("rejected by user")
        // Safe Wallet via WalletConnect: code -32000
        || msg.contains("-32000")
        // MetaMask EIP-5792 bundle rejection
        || msg.contains("unknownbundleid")
}

fn is_7702_upgrade_rejection(msg: &str) -> bool {
    let is_tx_error = msg.contains("transaction") && msg.contains("error");
    let has_rejected_upgrade = msg.contains("rejected") && msg.contains("upgrade");
    let has_7702 = msg.contains("7702");
    is_tx_error && (has_rejected_upgrade || has_7702)
}
