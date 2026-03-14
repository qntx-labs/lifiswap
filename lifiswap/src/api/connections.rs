//! `GET /connections` endpoint.

use crate::client::LiFiClient;
use crate::error::Result;
use crate::types::{ConnectionsRequest, ConnectionsResponse};

impl LiFiClient {
    /// Get all available connections for swap/bridging tokens.
    ///
    /// # Errors
    ///
    /// Returns [`LiFiError`](crate::error::LiFiError) on network or API errors.
    pub async fn get_connections(
        &self,
        params: &ConnectionsRequest,
    ) -> Result<ConnectionsResponse> {
        let mut url = url::Url::parse(&format!("{}/connections", self.api_url()))?;

        {
            let mut qs = url.query_pairs_mut();
            if let Some(ref fc) = params.from_chain {
                qs.append_pair("fromChain", &fc.to_string());
            }
            if let Some(ref ft) = params.from_token {
                qs.append_pair("fromToken", ft);
            }
            if let Some(ref tc) = params.to_chain {
                qs.append_pair("toChain", &tc.to_string());
            }
            if let Some(ref tt) = params.to_token {
                qs.append_pair("toToken", tt);
            }
            if let Some(ref v) = params.allow_bridges {
                for item in v {
                    qs.append_pair("allowBridges", item);
                }
            }
            if let Some(ref v) = params.deny_bridges {
                for item in v {
                    qs.append_pair("denyBridges", item);
                }
            }
            if let Some(ref v) = params.prefer_bridges {
                for item in v {
                    qs.append_pair("preferBridges", item);
                }
            }
            if let Some(ref v) = params.allow_exchanges {
                for item in v {
                    qs.append_pair("allowExchanges", item);
                }
            }
            if let Some(ref v) = params.deny_exchanges {
                for item in v {
                    qs.append_pair("denyExchanges", item);
                }
            }
            if let Some(ref v) = params.prefer_exchanges {
                for item in v {
                    qs.append_pair("preferExchanges", item);
                }
            }
        }

        self.get_url(&url).await
    }
}
