//! Shared application context passed to every command handler.

use anyhow::Result;
use lifiswap::{LiFiClient, LiFiConfig};

use crate::output::OutputFormat;

/// Shared state available to all CLI commands.
pub struct App {
    /// The configured `LiFi` SDK client.
    pub client: LiFiClient,
    /// Chosen output format (table / json / compact).
    pub output: OutputFormat,
}

impl App {
    /// Build an `App` from parsed CLI global arguments.
    pub fn new(
        integrator: &str,
        api_key: Option<&str>,
        api_url: Option<&str>,
        output: OutputFormat,
    ) -> Result<Self> {
        let config = LiFiConfig::builder()
            .integrator(integrator)
            .maybe_api_key(api_key)
            .maybe_api_url(api_url)
            .build();

        let client = LiFiClient::new(config)?;
        Ok(Self { client, output })
    }
}
