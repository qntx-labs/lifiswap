//! Solana-specific execution tasks.

mod send_confirm;
mod sign;

pub use send_confirm::SvmSendAndConfirmTask;
pub use sign::SvmSignTask;

/// Build a block-explorer transaction link from chain metadata.
///
/// Returns `None` if the chain has no configured explorer URLs.
pub fn get_tx_link(chain: &lifiswap::types::Chain, tx_sig: &str) -> Option<String> {
    let urls = chain.metamask.as_ref()?.block_explorer_urls.as_ref()?;
    let base = urls.first()?;
    let base = base.trim_end_matches('/');
    Some(format!("{base}/tx/{tx_sig}"))
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}
