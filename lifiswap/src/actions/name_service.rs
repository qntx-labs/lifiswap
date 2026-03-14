//! Name service address resolution.
//!
//! Mirrors the `TypeScript` SDK's `getNameServiceAddress` action.

use std::sync::Arc;

use crate::LiFiClient;
use crate::provider::Provider;
use crate::types::ChainType;

impl LiFiClient {
    /// Resolve a human-readable name (e.g. ENS, SNS) to an on-chain address.
    ///
    /// Tries each registered provider that matches the optional `chain_type` filter.
    /// Returns the first successful resolution, or `None` if no provider
    /// can resolve the name.
    ///
    /// # Panics
    ///
    /// Panics if the internal providers lock is poisoned.
    pub async fn get_name_service_address(
        &self,
        name: &str,
        chain_type: Option<ChainType>,
    ) -> Option<String> {
        let filtered: Vec<Arc<dyn Provider>> = {
            let providers = self
                .inner
                .providers
                .read()
                .expect("providers lock poisoned");
            chain_type.map_or_else(
                || providers.clone(),
                |ct| {
                    providers
                        .iter()
                        .filter(|p| p.chain_type() == ct)
                        .cloned()
                        .collect()
                },
            )
        };

        for provider in &filtered {
            if let Ok(Some(address)) = provider.resolve_address(name, None).await {
                return Some(address);
            }
        }

        None
    }
}
