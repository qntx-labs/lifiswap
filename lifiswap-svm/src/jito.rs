//! Jito Bundle Engine client for MEV-protected transaction submission.
//!
//! Provides [`JitoClient`] for sending Solana transactions as Jito bundles
//! and polling for bundle confirmation status. Mirrors the TS SDK's
//! `sendAndConfirmBundle` flow.

use std::sync::Arc;

use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use serde::{Deserialize, Serialize};
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::signature::Signature;

/// Default Jito Block Engine endpoint.
const DEFAULT_JITO_URL: &str = "https://mainnet.block-engine.jito.wtf/api/v1";

/// Jito bundle client for submitting and confirming transaction bundles.
///
/// Uses the Jito Block Engine JSON-RPC API (`sendBundle`, `getBundleStatuses`)
/// alongside a standard Solana RPC for blockhash and block height queries.
#[derive(Clone)]
pub struct JitoClient {
    http: reqwest::Client,
    jito_url: String,
    rpc: Arc<RpcClient>,
}

impl std::fmt::Debug for JitoClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JitoClient")
            .field("jito_url", &self.jito_url)
            .finish_non_exhaustive()
    }
}

impl JitoClient {
    /// Create a new Jito client with a standard RPC for auxiliary queries.
    #[must_use]
    pub fn new(rpc: Arc<RpcClient>) -> Self {
        Self {
            http: reqwest::Client::new(),
            jito_url: DEFAULT_JITO_URL.to_owned(),
            rpc,
        }
    }

    /// Create a Jito client with a custom Block Engine URL.
    #[must_use]
    pub fn with_url(rpc: Arc<RpcClient>, jito_url: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            jito_url: jito_url.into(),
            rpc,
        }
    }

    /// Submit a bundle of base64-encoded transactions to the Jito Block Engine.
    ///
    /// # Errors
    ///
    /// Returns an error if the submission request fails.
    pub async fn send_bundle(&self, base64_txs: &[String]) -> Result<String> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "sendBundle",
            params: (base64_txs,),
        };

        let resp: JsonRpcResponse<String> = self
            .http
            .post(format!("{}/bundles", self.jito_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| jito_error(&e))?
            .error_for_status()
            .map_err(|e| jito_error(&e))?
            .json()
            .await
            .map_err(|e| jito_error(&e))?;

        resp.result.ok_or_else(|| LiFiError::Transaction {
            code: LiFiErrorCode::TransactionFailed,
            message: format!(
                "Jito sendBundle failed: {}",
                resp.error
                    .map_or_else(|| "unknown error".to_owned(), |e| e.message)
            ),
        })
    }

    /// Get the status of a bundle by its ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the status request fails.
    pub async fn get_bundle_statuses(&self, bundle_ids: &[&str]) -> Result<Vec<BundleStatusEntry>> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "getBundleStatuses",
            params: (bundle_ids,),
        };

        let resp: JsonRpcResponse<BundleStatusResult> = self
            .http
            .post(format!("{}/bundles", self.jito_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| jito_error(&e))?
            .error_for_status()
            .map_err(|e| jito_error(&e))?
            .json()
            .await
            .map_err(|e| jito_error(&e))?;

        Ok(resp.result.map_or_else(Vec::new, |r| r.value))
    }

    /// Returns a reference to the underlying Solana RPC client.
    #[must_use]
    pub fn rpc(&self) -> &RpcClient {
        &self.rpc
    }

    /// Send a bundle and poll for confirmation.
    ///
    /// Submits the base64-encoded transactions, then polls `getBundleStatuses`
    /// until the bundle is confirmed/finalized or the blockhash expires.
    ///
    /// # Errors
    ///
    /// Returns an error if submission or confirmation fails.
    pub async fn send_and_confirm_bundle(&self, base64_txs: &[String]) -> Result<BundleResult> {
        let bundle_id = self.send_bundle(base64_txs).await?;

        let commitment = solana_commitment_config::CommitmentConfig::confirmed();
        let (_, last_valid_block_height) = self
            .rpc
            .get_latest_blockhash_with_commitment(commitment)
            .await
            .map_err(|e| LiFiError::Provider {
                code: LiFiErrorCode::ProviderUnavailable,
                message: format!("Failed to get blockhash: {e}"),
            })?;

        loop {
            let statuses = self.get_bundle_statuses(&[&bundle_id]).await?;

            if let Some(status) = statuses.first() {
                let confirmed = status.confirmation_status == "confirmed"
                    || status.confirmation_status == "finalized";

                if confirmed {
                    let tx_signatures: Vec<Signature> = status
                        .transactions
                        .iter()
                        .filter_map(|s| s.parse().ok())
                        .collect();

                    return Ok(BundleResult {
                        bundle_id: bundle_id.clone(),
                        tx_signatures,
                    });
                }
            }

            // Check block height expiry
            let current_height = self.rpc.get_block_height().await.unwrap_or(0);
            if current_height > last_valid_block_height {
                return Err(LiFiError::Transaction {
                    code: LiFiErrorCode::TransactionExpired,
                    message: "Bundle expired: block height exceeded.".to_owned(),
                });
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
        }
    }
}

fn jito_error(e: &reqwest::Error) -> LiFiError {
    LiFiError::Provider {
        code: LiFiErrorCode::ProviderUnavailable,
        message: format!("Jito API error: {e}"),
    }
}

/// Result of a confirmed Jito bundle.
#[derive(Debug, Clone)]
pub struct BundleResult {
    /// The Jito bundle ID.
    pub bundle_id: String,
    /// Transaction signatures from the confirmed bundle.
    pub tx_signatures: Vec<Signature>,
}

#[derive(Serialize)]
struct JsonRpcRequest<'a, P> {
    jsonrpc: &'a str,
    id: u64,
    method: &'a str,
    params: P,
}

#[derive(Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize)]
struct JsonRpcError {
    message: String,
}

#[derive(Deserialize)]
struct BundleStatusResult {
    value: Vec<BundleStatusEntry>,
}

/// Status of a single Jito bundle.
#[derive(Debug, Clone, Deserialize)]
pub struct BundleStatusEntry {
    /// Bundle ID.
    pub bundle_id: String,
    /// Transaction signatures in the bundle.
    pub transactions: Vec<String>,
    /// Confirmation status: "processed", "confirmed", or "finalized".
    pub confirmation_status: String,
}
