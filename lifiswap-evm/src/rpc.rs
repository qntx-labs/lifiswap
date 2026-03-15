//! RPC URL resolution for multi-chain support.
//!
//! The [`RpcUrlResolver`] trait allows dynamic RPC endpoint resolution
//! based on chain ID, enabling a single [`EvmProvider`](crate::EvmProvider)
//! to interact with multiple EVM chains.

use std::collections::HashMap;

/// Resolves RPC endpoint URLs by chain ID.
///
/// Implementations can provide static mappings, load-balanced endpoints,
/// or runtime-configurable URLs.
///
/// # Example
///
/// ```ignore
/// use lifiswap_evm::rpc::StaticRpcUrls;
///
/// let resolver = StaticRpcUrls::new([
///     (1, "https://eth.llamarpc.com".parse().unwrap()),
///     (42161, "https://arb1.arbitrum.io/rpc".parse().unwrap()),
/// ]);
/// ```
pub trait RpcUrlResolver: Send + Sync + std::fmt::Debug + 'static {
    /// Resolve the RPC URL for the given chain ID.
    ///
    /// Returns `None` if the chain is not supported by this resolver.
    fn resolve(&self, chain_id: u64) -> Option<url::Url>;
}

/// Static RPC URL resolver backed by a `HashMap`.
#[derive(Debug, Clone)]
pub struct StaticRpcUrls {
    urls: HashMap<u64, url::Url>,
}

impl StaticRpcUrls {
    /// Create a new resolver from an iterator of `(chain_id, url)` pairs.
    pub fn new(urls: impl IntoIterator<Item = (u64, url::Url)>) -> Self {
        Self {
            urls: urls.into_iter().collect(),
        }
    }
}

impl RpcUrlResolver for StaticRpcUrls {
    fn resolve(&self, chain_id: u64) -> Option<url::Url> {
        self.urls.get(&chain_id).cloned()
    }
}
