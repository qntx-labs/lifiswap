//! Error types for the `LiFi` SDK.
//!
//! Maps error codes from the `TypeScript` SDK's `errors/constants.ts` and provides
//! a unified [`LiFiError`] enum for all SDK operations.

use std::fmt;
use std::time::Duration;

/// `LiFi` error codes, aligned with the `TypeScript` SDK.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum LiFiErrorCode {
    /// Internal SDK error.
    InternalError = 1000,
    /// Request validation failed.
    ValidationError = 1001,
    /// Transaction was underpriced.
    TransactionUnderpriced = 1002,
    /// Transaction execution failed.
    TransactionFailed = 1003,
    /// Operation timed out.
    Timeout = 1004,
    /// Provider not available for the given chain.
    ProviderUnavailable = 1005,
    /// Requested resource not found.
    NotFound = 1006,
    /// Insufficient funds/balance.
    InsufficientFunds = 1007,
    /// Chain switch required but not allowed.
    ChainSwitchError = 1008,
    /// Wallet changed during execution.
    WalletChangedDuringExecution = 1009,
    /// Transaction was rejected by the user.
    TransactionRejected = 1010,
    /// Slippage exceeded the allowed threshold.
    SlippageError = 1011,
    /// Transaction was cancelled.
    TransactionCancelled = 1012,
    /// SDK configuration error.
    ConfigError = 1013,
    /// Allowance is not sufficient.
    AllowanceRequired = 1014,
    /// Exchange rate expired.
    ExchangeRateExpired = 1015,
    /// RPC communication error.
    RpcError = 1016,
    /// Server-side error from the `LiFi` API.
    ServerError = 1100,
}

impl fmt::Display for LiFiErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", *self as u16)
    }
}

/// HTTP error details from a `LiFi` API response.
#[derive(Debug, Clone)]
pub struct HttpErrorDetails {
    /// HTTP status code.
    pub status: u16,
    /// Response body (may contain JSON error details).
    pub body: String,
    /// Mapped `LiFi` error code.
    pub code: LiFiErrorCode,
    /// Server-suggested retry delay from `Retry-After` header (429 responses).
    pub retry_after: Option<Duration>,
}

impl fmt::Display for HttpErrorDetails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HTTP {} (code {}): {}",
            self.status, self.code, self.body
        )
    }
}

/// Map an HTTP status code to a [`LiFiErrorCode`].
#[must_use]
pub const fn http_status_to_error_code(status: u16) -> LiFiErrorCode {
    match status {
        400 => LiFiErrorCode::ValidationError,
        401 | 403 => LiFiErrorCode::ConfigError,
        404 => LiFiErrorCode::NotFound,
        409 => LiFiErrorCode::SlippageError,
        429 | 500..=599 => LiFiErrorCode::ServerError,
        _ => LiFiErrorCode::InternalError,
    }
}

/// The unified error type for all `LiFi` SDK operations.
#[derive(Debug, thiserror::Error)]
pub enum LiFiError {
    /// HTTP error from the `LiFi` API.
    #[error("HTTP error: {0}")]
    Http(HttpErrorDetails),

    /// Request parameter validation failed.
    #[error("Validation error: {0}")]
    Validation(String),

    /// Provider-related error (missing provider, chain not supported, etc.).
    #[error("Provider error (code {code}): {message}")]
    Provider {
        /// Error code.
        code: LiFiErrorCode,
        /// Error message.
        message: String,
    },

    /// On-chain transaction error.
    #[error("Transaction error (code {code}): {message}")]
    Transaction {
        /// Error code.
        code: LiFiErrorCode,
        /// Error message.
        message: String,
    },

    /// Insufficient token balance.
    #[error("Balance error: {0}")]
    Balance(String),

    /// Route execution error.
    #[error("Execution error: {0}")]
    Execution(String),

    /// JSON serialization/deserialization error.
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    /// Network/transport error from reqwest.
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// URL parsing error.
    #[error("URL error: {0}")]
    Url(#[from] url::ParseError),

    /// Server-side error with API error code.
    #[error("Server error (code {code}): {message}")]
    Server {
        /// Error code from the server.
        code: i32,
        /// Error message from the server.
        message: String,
    },

    /// SDK configuration error (missing integrator, invalid API URL, etc.).
    #[error("Config error: {0}")]
    Config(String),
}

impl LiFiError {
    /// Whether this error is transient and the request should be retried.
    ///
    /// Returns `true` for server errors (5xx), rate limits (429), and
    /// network-level failures that are likely transient.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Http(details) => matches!(details.status, 429 | 500..=599),
            Self::Network(e) => e.is_timeout() || e.is_connect(),
            _ => false,
        }
    }

    /// Returns the server-suggested retry delay if this is a 429 response
    /// with a `Retry-After` header.
    #[must_use]
    pub const fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Http(details) => details.retry_after,
            _ => None,
        }
    }
}

/// A type alias for `Result` with [`LiFiError`].
pub type Result<T> = std::result::Result<T, LiFiError>;
