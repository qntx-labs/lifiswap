//! Bitcoin-specific execution tasks.

mod confirm;
mod sign;

pub use confirm::BtcConfirmTask;
pub use sign::BtcSignTask;

/// Input outpoints from the original transaction, shared between
/// [`BtcSignTask`] and [`BtcConfirmTask`] for RBF replacement detection.
///
/// After signing, `BtcSignTask` stores the first input outpoint so that
/// `BtcConfirmTask` can detect if the inputs were spent by a different
/// (replacement) transaction.
#[derive(Debug, Default)]
pub struct BtcTxInputs {
    /// First input outpoint `(prev_txid, prev_vout)` from the broadcast tx.
    pub first_input: std::sync::Mutex<Option<(String, u32)>>,
}

/// Construct a block explorer transaction link.
pub fn get_tx_link(chain: &lifiswap::types::Chain, tx_hash: &str) -> Option<String> {
    chain
        .metamask
        .as_ref()
        .and_then(|m| m.block_explorer_urls.as_ref())
        .and_then(|urls| urls.first())
        .map(|url| format!("{url}tx/{tx_hash}"))
}

/// Current timestamp in milliseconds.
#[allow(clippy::cast_possible_truncation)]
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
