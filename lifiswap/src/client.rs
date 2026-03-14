//! `LiFi` SDK client and configuration.
//!
//! The [`LiFiClient`] is the main entry point for interacting with the `LiFi` API.
//! Use [`LiFiClientBuilder`] (via [`LiFiClient::builder`]) to construct a configured client.
//!
//! # Example
//!
//! ```no_run
//! use lifiswap::{LiFiClient, types::ChainsRequest};
//!
//! # async fn example() -> lifiswap::error::Result<()> {
//! let client = LiFiClient::builder()
//!     .integrator("my-app")
//!     .build()?;
//!
//! let chains = client.get_chains(None).await?;
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;

use crate::error::{LiFiError, Result};
use crate::http::{DEFAULT_API_URL, HttpConfig};
use crate::types::{ChainId, RouteOptions};

/// Configuration for the `LiFi` SDK client.
#[derive(Debug, Clone)]
pub struct LiFiConfig {
    /// `LiFi` API base URL (default: `https://li.quest/v1`).
    pub api_url: String,
    /// Optional API key for authentication.
    pub api_key: Option<String>,
    /// Integrator identifier (required by the `LiFi` API).
    pub integrator: String,
    /// Optional user identifier.
    pub user_id: Option<String>,
    /// Default route options applied to quote/route requests.
    pub route_options: Option<RouteOptions>,
    /// Custom RPC URLs per chain.
    pub rpc_urls: HashMap<ChainId, Vec<String>>,
}

/// Builder for constructing a [`LiFiClient`] with desired configuration.
#[derive(Debug, Default)]
pub struct LiFiClientBuilder {
    api_url: Option<String>,
    api_key: Option<String>,
    integrator: Option<String>,
    user_id: Option<String>,
    route_options: Option<RouteOptions>,
    rpc_urls: HashMap<ChainId, Vec<String>>,
}

impl LiFiClientBuilder {
    /// Set the API base URL (default: `https://li.quest/v1`).
    #[must_use]
    pub fn api_url(mut self, url: impl Into<String>) -> Self {
        self.api_url = Some(url.into());
        self
    }

    /// Set the API key for authentication.
    #[must_use]
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set the integrator identifier (required).
    #[must_use]
    pub fn integrator(mut self, integrator: impl Into<String>) -> Self {
        self.integrator = Some(integrator.into());
        self
    }

    /// Set a user identifier sent with requests.
    #[must_use]
    pub fn user_id(mut self, id: impl Into<String>) -> Self {
        self.user_id = Some(id.into());
        self
    }

    /// Set default route options.
    #[must_use]
    pub fn route_options(mut self, opts: RouteOptions) -> Self {
        self.route_options = Some(opts);
        self
    }

    /// Add a custom RPC URL for a chain.
    #[must_use]
    pub fn rpc_url(mut self, chain_id: impl Into<ChainId>, url: impl Into<String>) -> Self {
        self.rpc_urls
            .entry(chain_id.into())
            .or_default()
            .push(url.into());
        self
    }

    /// Build the [`LiFiClient`].
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError::Config`] if the `integrator` field is not set.
    pub fn build(self) -> Result<LiFiClient> {
        let integrator = self.integrator.ok_or_else(|| {
            LiFiError::Config("integrator is required — call .integrator(\"your-app\")".into())
        })?;

        let config = LiFiConfig {
            api_url: self.api_url.unwrap_or_else(|| DEFAULT_API_URL.to_owned()),
            api_key: self.api_key,
            integrator,
            user_id: self.user_id,
            route_options: self.route_options,
            rpc_urls: self.rpc_urls,
        };

        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(LiFiError::Network)?;

        Ok(LiFiClient {
            config,
            http: http_client,
        })
    }
}

/// The `LiFi` SDK client.
///
/// Provides methods for all `LiFi` REST API endpoints. Thread-safe and cloneable.
#[derive(Debug, Clone)]
pub struct LiFiClient {
    pub(crate) config: LiFiConfig,
    pub(crate) http: reqwest::Client,
}

impl LiFiClient {
    /// Create a new [`LiFiClientBuilder`].
    #[must_use]
    pub fn builder() -> LiFiClientBuilder {
        LiFiClientBuilder::default()
    }

    /// Returns a reference to the current configuration.
    #[must_use]
    pub const fn config(&self) -> &LiFiConfig {
        &self.config
    }

    /// Returns the internal [`HttpConfig`] used for requests.
    pub(crate) fn http_config(&self) -> HttpConfig {
        HttpConfig {
            api_url: self.config.api_url.clone(),
            integrator: self.config.integrator.clone(),
            api_key: self.config.api_key.clone(),
            user_id: self.config.user_id.clone(),
        }
    }
}
