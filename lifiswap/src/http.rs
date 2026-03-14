//! Internal HTTP request layer.
//!
//! Wraps [`reqwest`] with `LiFi`-specific header injection, automatic retries on
//! server errors, and error mapping.

use std::time::Duration;

use reqwest::{Client, Method, Response};
use serde::de::DeserializeOwned;

use crate::error::{HttpErrorDetails, LiFiError, http_status_to_error_code};

/// SDK version string sent in the `x-lifi-sdk` header.
const SDK_VERSION: &str = concat!("lifiswap-rs/", env!("CARGO_PKG_VERSION"));

/// Default API base URL.
pub const DEFAULT_API_URL: &str = "https://li.quest/v1";

/// Maximum number of retries for server errors (HTTP 500+).
const MAX_RETRIES: u32 = 1;

/// Delay between retries.
const RETRY_DELAY: Duration = Duration::from_millis(500);

/// Configuration consumed by the HTTP layer.
#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub api_url: String,
    pub api_key: Option<String>,
    pub integrator: String,
    pub user_id: Option<String>,
}

/// Send a GET request and deserialize the JSON response.
///
/// # Errors
///
/// Returns [`LiFiError::Http`] on non-2xx responses and [`LiFiError::Network`]
/// on transport-level failures.
pub async fn get<T: DeserializeOwned>(
    client: &Client,
    config: &HttpConfig,
    url: &str,
) -> crate::error::Result<T> {
    request(client, config, Method::GET, url, None).await
}

/// Send a POST request with a JSON body and deserialize the response.
///
/// # Errors
///
/// Returns [`LiFiError::Http`] on non-2xx responses and [`LiFiError::Network`]
/// on transport-level failures.
pub async fn post<T: DeserializeOwned>(
    client: &Client,
    config: &HttpConfig,
    url: &str,
    body: &(impl serde::Serialize + Sync),
) -> crate::error::Result<T> {
    let json = serde_json::to_string(body)?;
    request(client, config, Method::POST, url, Some(&json)).await
}

/// Core request function with header injection and retry logic.
async fn request<T: DeserializeOwned>(
    client: &Client,
    config: &HttpConfig,
    method: Method,
    url: &str,
    body: Option<&str>,
) -> crate::error::Result<T> {
    let mut last_error: Option<LiFiError> = None;
    let attempts = MAX_RETRIES + 1;

    for attempt in 0..attempts {
        if attempt > 0 {
            tracing::debug!(attempt, url, "retrying request");
            tokio::time::sleep(RETRY_DELAY).await;
        }

        let builder = build_request(client, config, &method, url, body);
        let result = builder.send().await;

        match result {
            Ok(response) => {
                let status = response.status().as_u16();
                if response.status().is_success() {
                    let text = response.text().await.map_err(LiFiError::Network)?;
                    let parsed: T = serde_json::from_str(&text)?;
                    return Ok(parsed);
                }

                // Retry on 500+ server errors.
                if status >= 500 && attempt < MAX_RETRIES {
                    let body_text = response.text().await.unwrap_or_default();
                    tracing::warn!(status, attempt, url, "server error, will retry");
                    last_error = Some(LiFiError::Http(HttpErrorDetails {
                        status,
                        body: body_text,
                        code: http_status_to_error_code(status),
                    }));
                    continue;
                }

                return Err(handle_error_response(response).await);
            }
            Err(e) => {
                if attempt < MAX_RETRIES {
                    tracing::warn!(attempt, url, error = %e, "network error, will retry");
                    last_error = Some(LiFiError::Network(e));
                    continue;
                }
                return Err(LiFiError::Network(e));
            }
        }
    }

    // Should not be reachable, but return last error as safety net.
    Err(last_error.unwrap_or_else(|| LiFiError::Execution("request exhausted retries".into())))
}

/// Build a request with `LiFi` headers injected.
fn build_request(
    client: &Client,
    config: &HttpConfig,
    method: &Method,
    url: &str,
    body: Option<&str>,
) -> reqwest::RequestBuilder {
    let mut builder = client.request(method.clone(), url);

    builder = builder
        .header("x-lifi-sdk", SDK_VERSION)
        .header("x-lifi-integrator", &config.integrator);

    if let Some(ref key) = config.api_key {
        builder = builder.header("x-lifi-api-key", key);
    }

    if let Some(ref uid) = config.user_id {
        builder = builder.header("x-lifi-userid", uid);
    }

    if let Some(json_body) = body {
        builder = builder
            .header("Content-Type", "application/json")
            .body(json_body.to_owned());
    }

    builder
}

/// Extract error details from a non-2xx HTTP response.
async fn handle_error_response(response: Response) -> LiFiError {
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    let code = http_status_to_error_code(status);

    tracing::debug!(status, code = ?code, "API error response");

    LiFiError::Http(HttpErrorDetails { status, body, code })
}
