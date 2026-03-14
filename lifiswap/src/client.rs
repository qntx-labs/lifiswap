//! `LiFi` SDK client and configuration.
//!
//! The [`LiFiClient`] is the main entry point for interacting with the `LiFi` API.
//! Construct one via [`LiFiClient::new`] (using [`LiFiConfig::builder`]).
//!
//! # Example
//!
//! ```no_run
//! use lifiswap::LiFiClient;
//! use lifiswap::client::LiFiConfig;
//!
//! # async fn example() -> lifiswap::error::Result<()> {
//! let client = LiFiClient::new(
//!     LiFiConfig::builder()
//!         .integrator("my-app")
//!         .build(),
//! )?;
//!
//! let chains = client.get_chains(None).await?;
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue};

use crate::error::{LiFiError, Result};
use crate::types::RouteOptions;

/// SDK version sent in the `x-lifi-sdk` header.
const SDK_VERSION: &str = concat!("lifiswap-rs/", env!("CARGO_PKG_VERSION"));

/// Default API base URL.
pub const DEFAULT_API_URL: &str = "https://li.quest/v1";

/// Retry configuration for transient failures.
///
/// Uses exponential backoff with optional jitter via [`backon`].
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
pub struct RetryConfig {
    /// Maximum number of retry attempts (default: 3).
    #[builder(default = 3)]
    pub max_retries: usize,
    /// Minimum delay between retries (default: 300ms).
    #[builder(default = Duration::from_millis(300))]
    pub min_delay: Duration,
    /// Maximum delay cap (default: 10s).
    #[builder(default = Duration::from_secs(10))]
    pub max_delay: Duration,
    /// Whether to add jitter to delays (default: true).
    #[builder(default = true)]
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            min_delay: Duration::from_millis(300),
            max_delay: Duration::from_secs(10),
            jitter: true,
        }
    }
}

/// `LiFi` SDK client configuration.
///
/// Use [`LiFiConfig::builder()`] for ergonomic construction:
///
/// ```
/// use lifiswap::client::LiFiConfig;
///
/// let config = LiFiConfig::builder()
///     .integrator("my-app")
///     .api_key("sk-...")
///     .build();
/// ```
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
pub struct LiFiConfig {
    /// Integrator identifier (**required** by the `LiFi` API).
    #[builder(into)]
    pub integrator: String,
    /// API base URL (default: `https://li.quest/v1`).
    #[builder(into, default = DEFAULT_API_URL.to_owned())]
    pub api_url: String,
    /// Optional API key for authenticated endpoints.
    #[builder(into)]
    pub api_key: Option<String>,
    /// Optional user identifier sent with requests.
    #[builder(into)]
    pub user_id: Option<String>,
    /// Default route options applied to quote/route requests.
    pub route_options: Option<RouteOptions>,
    /// Retry policy for transient failures.
    #[builder(default)]
    pub retry: RetryConfig,
    /// Per-request timeout (default: 30s).
    #[builder(default = Duration::from_secs(30))]
    pub timeout: Duration,
}

/// Shared inner state behind `Arc`.
#[derive(Debug)]
pub(crate) struct ClientInner {
    pub(crate) config: LiFiConfig,
    pub(crate) http: reqwest::Client,
}

impl ClientInner {
    /// Build a [`backon::ExponentialBuilder`] from the retry config.
    pub(crate) fn backoff(&self) -> backon::ExponentialBuilder {
        let mut b = backon::ExponentialBuilder::default()
            .with_min_delay(self.config.retry.min_delay)
            .with_max_delay(self.config.retry.max_delay)
            .with_max_times(self.config.retry.max_retries);
        if self.config.retry.jitter {
            b = b.with_jitter();
        }
        b
    }
}

/// The `LiFi` SDK client.
///
/// Cheaply cloneable (`Arc`-backed). Thread-safe (`Send + Sync`).
/// Provides methods for all `LiFi` REST API endpoints.
#[derive(Debug, Clone)]
pub struct LiFiClient {
    pub(crate) inner: Arc<ClientInner>,
}

impl LiFiClient {
    /// Create a new client from a [`LiFiConfig`].
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError::Network`] if the underlying HTTP client fails to initialize.
    pub fn new(config: LiFiConfig) -> Result<Self> {
        let headers = Self::build_headers(&config);

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(config.timeout)
            .pool_max_idle_per_host(20)
            .build()
            .map_err(LiFiError::Network)?;

        Ok(Self {
            inner: Arc::new(ClientInner { config, http }),
        })
    }

    /// Create a client with a pre-configured [`reqwest::Client`].
    ///
    /// Use this when you need custom middleware, proxy settings, or TLS
    /// configuration that the default builder doesn't expose.
    ///
    /// **Note:** SDK headers (`x-lifi-sdk`, `x-lifi-integrator`, etc.)
    /// are **not** automatically injected — you must set them yourself
    /// if the provided client doesn't already include them.
    #[must_use]
    pub fn with_http_client(config: LiFiConfig, http: reqwest::Client) -> Self {
        Self {
            inner: Arc::new(ClientInner { config, http }),
        }
    }

    /// Build the default SDK headers from a config.
    fn build_headers(config: &LiFiConfig) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-lifi-sdk", HeaderValue::from_static(SDK_VERSION));
        if let Ok(v) = HeaderValue::from_str(&config.integrator) {
            headers.insert("x-lifi-integrator", v);
        }
        if let Some(ref key) = config.api_key
            && let Ok(v) = HeaderValue::from_str(key)
        {
            headers.insert("x-lifi-api-key", v);
        }
        if let Some(ref uid) = config.user_id
            && let Ok(v) = HeaderValue::from_str(uid)
        {
            headers.insert("x-lifi-userid", v);
        }
        headers
    }

    /// Returns a reference to the current configuration.
    #[must_use]
    pub fn config(&self) -> &LiFiConfig {
        &self.inner.config
    }

    /// Returns the API base URL.
    #[must_use]
    pub fn api_url(&self) -> &str {
        &self.inner.config.api_url
    }
}
