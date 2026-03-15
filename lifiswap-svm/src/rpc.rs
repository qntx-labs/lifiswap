//! Multi-RPC management with sequential retry.
//!
//! Mirrors the TS SDK's `rpc/registry.ts` and `rpc/utils.ts`:
//! maintains a set of Solana RPC clients and retries operations across
//! them sequentially until one succeeds.

use std::future::Future;
use std::sync::Arc;

use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use solana_rpc_client::nonblocking::rpc_client::RpcClient;

/// A pool of Solana RPC clients for redundant access.
///
/// Operations are attempted on each client in order; the first
/// successful result is returned. If all fail, an aggregate error
/// is returned.
#[derive(Clone)]
pub struct RpcPool {
    clients: Vec<Arc<RpcClient>>,
}

impl std::fmt::Debug for RpcPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcPool")
            .field("count", &self.clients.len())
            .finish()
    }
}

impl RpcPool {
    /// Create a pool from a list of RPC endpoint URLs.
    ///
    /// # Errors
    ///
    /// Returns an error if no URLs are provided.
    pub fn new(urls: &[url::Url]) -> Result<Self> {
        if urls.is_empty() {
            return Err(LiFiError::Config(
                "At least one Solana RPC URL is required".to_owned(),
            ));
        }
        let clients = urls
            .iter()
            .map(|u| Arc::new(RpcClient::new(u.to_string())))
            .collect();
        Ok(Self { clients })
    }

    /// Create a pool from a single RPC URL.
    #[must_use]
    pub fn from_single(url: &url::Url) -> Self {
        Self {
            clients: vec![Arc::new(RpcClient::new(url.to_string()))],
        }
    }

    /// Returns the number of RPC clients in the pool.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.clients.len()
    }

    /// Returns `true` if the pool contains no clients.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.clients.is_empty()
    }

    /// Execute an async operation across all RPCs with sequential fallback.
    ///
    /// Tries each RPC in order. Returns the first successful result.
    /// If all RPCs fail, returns the last error.
    ///
    /// # Errors
    ///
    /// Returns the last encountered error if all RPCs fail.
    pub async fn call_with_retry<F, Fut, R>(&self, f: F) -> Result<R>
    where
        F: Fn(Arc<RpcClient>) -> Fut,
        Fut: Future<Output = Result<R>>,
    {
        let mut last_error = None;
        for client in &self.clients {
            match f(Arc::clone(client)).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::debug!(error = %e, "RPC call failed, trying next");
                    last_error = Some(e);
                }
            }
        }
        Err(last_error.unwrap_or_else(|| LiFiError::Provider {
            code: LiFiErrorCode::ProviderUnavailable,
            message: "No RPCs available".to_owned(),
        }))
    }
}
