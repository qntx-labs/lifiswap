//! Name service address resolution.
//!
//! Mirrors the `TypeScript` SDK's `getNameServiceAddress` action.

use crate::LiFiClient;
use crate::provider::Provider;
use crate::types::ChainType;

impl LiFiClient {
    /// Resolve a human-readable name (e.g. ENS, SNS) to an on-chain address.
    ///
    /// Tries each provider that matches the optional `chain_type` filter.
    /// Returns the first successful resolution, or `None` if no provider
    /// can resolve the name.
    pub async fn get_name_service_address(
        &self,
        name: &str,
        chain_type: Option<ChainType>,
        providers: &[Box<dyn Provider>],
    ) -> Option<String> {
        let filtered: Vec<&dyn Provider> = if let Some(ct) = chain_type {
            providers
                .iter()
                .filter(|p| p.chain_type() == ct)
                .map(AsRef::as_ref)
                .collect()
        } else {
            providers.iter().map(AsRef::as_ref).collect()
        };

        for provider in filtered {
            match provider.resolve_address(name, None).await {
                Ok(Some(address)) => return Some(address),
                Ok(None) | Err(_) => continue,
            }
        }

        None
    }
}
