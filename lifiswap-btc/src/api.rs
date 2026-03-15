//! Bitcoin blockchain REST API client.
//!
//! Provides [`BlockchainApi`] for querying balances, broadcasting transactions,
//! and checking confirmation status via public Bitcoin APIs (mempool.space, etc.).
//! Supports multiple backend URLs with sequential fallback, similar to the
//! TS SDK's bigmi client with blockchair/blockcypher/mempool transports.

use std::future::Future;

use lifiswap::error::{LiFiError, LiFiErrorCode, Result};

/// Default mempool.space API base URL.
const DEFAULT_API_URL: &str = "https://mempool.space/api";

/// Bitcoin blockchain REST API client with multi-backend fallback.
///
/// Operations are attempted on each backend URL in order; the first
/// successful result is returned.
#[derive(Clone, Debug)]
pub struct BlockchainApi {
    client: reqwest::Client,
    base_urls: Vec<String>,
}

impl BlockchainApi {
    /// Create a new API client with the default mempool.space backend.
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            base_urls: vec![DEFAULT_API_URL.to_owned()],
        }
    }

    /// Create a new API client with a custom HTTP client.
    #[must_use]
    pub fn with_client(client: reqwest::Client) -> Self {
        Self {
            client,
            base_urls: vec![DEFAULT_API_URL.to_owned()],
        }
    }

    /// Create with custom backend URLs for redundancy.
    ///
    /// # Errors
    ///
    /// Returns an error if no URLs are provided.
    pub fn with_urls(urls: Vec<String>) -> Result<Self> {
        if urls.is_empty() {
            return Err(LiFiError::Config(
                "At least one blockchain API URL is required".to_owned(),
            ));
        }
        Ok(Self {
            client: reqwest::Client::new(),
            base_urls: urls,
        })
    }

    /// Get the confirmed balance for an address in satoshis.
    ///
    /// Uses `GET /address/{addr}` and computes
    /// `chain_stats.funded_txo_sum - chain_stats.spent_txo_sum`.
    ///
    /// # Errors
    ///
    /// Returns an error if all backends fail.
    pub async fn get_balance(&self, address: &str) -> Result<u64> {
        self.call_with_retry(|base_url| {
            let url = format!("{base_url}/address/{address}");
            let client = &self.client;
            async move {
                let resp: AddressInfo = client
                    .get(&url)
                    .send()
                    .await
                    .map_err(api_error)?
                    .error_for_status()
                    .map_err(api_error)?
                    .json()
                    .await
                    .map_err(api_error)?;
                let funded = resp.chain_stats.funded_txo_sum;
                let spent = resp.chain_stats.spent_txo_sum;
                Ok(funded.saturating_sub(spent))
            }
        })
        .await
    }

    /// Get the current block height.
    ///
    /// # Errors
    ///
    /// Returns an error if all backends fail.
    pub async fn get_block_height(&self) -> Result<u64> {
        self.call_with_retry(|base_url| {
            let url = format!("{base_url}/blocks/tip/height");
            let client = &self.client;
            async move {
                let text = client
                    .get(&url)
                    .send()
                    .await
                    .map_err(api_error)?
                    .error_for_status()
                    .map_err(api_error)?
                    .text()
                    .await
                    .map_err(api_error)?;
                text.trim().parse::<u64>().map_err(|e| LiFiError::Provider {
                    code: LiFiErrorCode::ProviderUnavailable,
                    message: format!("Invalid block height response: {e}"),
                })
            }
        })
        .await
    }

    /// Broadcast a raw transaction hex and return the transaction ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the broadcast fails on all backends.
    pub async fn broadcast_tx(&self, hex: &str) -> Result<String> {
        self.call_with_retry(|base_url| {
            let url = format!("{base_url}/tx");
            let client = &self.client;
            let body = hex.to_owned();
            async move {
                let txid = client
                    .post(&url)
                    .body(body)
                    .send()
                    .await
                    .map_err(api_error)?
                    .error_for_status()
                    .map_err(api_error)?
                    .text()
                    .await
                    .map_err(api_error)?;
                Ok(txid.trim().to_owned())
            }
        })
        .await
    }

    /// Get the confirmation status of a transaction.
    ///
    /// # Errors
    ///
    /// Returns an error if all backends fail.
    pub async fn get_tx_status(&self, txid: &str) -> Result<TxStatus> {
        self.call_with_retry(|base_url| {
            let url = format!("{base_url}/tx/{txid}/status");
            let client = &self.client;
            async move {
                client
                    .get(&url)
                    .send()
                    .await
                    .map_err(api_error)?
                    .error_for_status()
                    .map_err(api_error)?
                    .json::<TxStatus>()
                    .await
                    .map_err(api_error)
            }
        })
        .await
    }

    /// Execute an async operation across all backends with sequential fallback.
    async fn call_with_retry<F, Fut, T>(&self, op: F) -> Result<T>
    where
        F: Fn(&str) -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let mut last_error = None;

        for base_url in &self.base_urls {
            match op(base_url).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::warn!(
                        base_url,
                        error = %e,
                        "Bitcoin API request failed, trying next backend"
                    );
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| LiFiError::Provider {
            code: LiFiErrorCode::ProviderUnavailable,
            message: "No Bitcoin API backends available".to_owned(),
        }))
    }
}

impl Default for BlockchainApi {
    fn default() -> Self {
        Self::new()
    }
}

fn api_error(e: reqwest::Error) -> LiFiError {
    LiFiError::Provider {
        code: LiFiErrorCode::ProviderUnavailable,
        message: format!("Bitcoin API error: {e}"),
    }
}

/// Address information returned by mempool.space `/address/{addr}`.
#[derive(Debug, serde::Deserialize)]
struct AddressInfo {
    chain_stats: ChainStats,
}

/// On-chain statistics for an address.
#[derive(Debug, serde::Deserialize)]
struct ChainStats {
    funded_txo_sum: u64,
    spent_txo_sum: u64,
}

/// Transaction confirmation status from mempool.space `/tx/{txid}/status`.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub struct TxStatus {
    /// Whether the transaction is confirmed.
    pub confirmed: bool,
    /// Block height at which the transaction was confirmed.
    pub block_height: Option<u64>,
}
