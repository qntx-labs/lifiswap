//! Wallet balance types.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::WalletTokenExtended;

/// Response from the wallet balances endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletBalancesResponse {
    /// Token balances grouped by chain ID.
    #[serde(default)]
    pub balances: HashMap<u64, Vec<WalletTokenExtended>>,
}
