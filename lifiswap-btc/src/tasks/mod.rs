//! Bitcoin-specific execution tasks.

mod confirm;
mod sign;

pub use confirm::BtcConfirmTask;
pub use sign::BtcSignTask;

/// Construct a block explorer transaction link.
pub(crate) fn get_tx_link(chain: &lifiswap::types::Chain, tx_hash: &str) -> Option<String> {
    chain
        .metamask
        .as_ref()
        .and_then(|m| m.block_explorer_urls.as_ref())
        .and_then(|urls| urls.first())
        .map(|url| format!("{url}tx/{tx_hash}"))
}

/// Current timestamp in milliseconds.
pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
