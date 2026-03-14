//! Internal HTTP request execution with retry.
//!
//! All HTTP requests flow through [`LiFiClient::get`] or [`LiFiClient::post`],
//! which provide automatic retries via [`backon`] with exponential backoff +
//! jitter, structured tracing, and consistent error mapping.

use std::time::Duration;

use backon::Retryable;
use reqwest::Response;
use serde::de::DeserializeOwned;

use crate::client::LiFiClient;
use crate::error::{HttpErrorDetails, LiFiError, http_status_to_error_code};

impl LiFiClient {
    /// Execute a GET request, deserializing the response as `T`.
    ///
    /// Query parameters are serialized from `query` via `serde_urlencoded`.
    pub(crate) async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &(impl serde::Serialize + Sync),
    ) -> crate::error::Result<T> {
        let url = format!("{}{path}", self.inner.config.api_url);
        let inner = &self.inner;

        (|| async {
            let resp = inner
                .http
                .get(&url)
                .query(query)
                .send()
                .await
                .map_err(LiFiError::Network)?;
            handle_response(resp).await
        })
        .retry(inner.backoff())
        .when(|e: &LiFiError| e.is_retryable())
        .notify(|err: &LiFiError, dur| {
            tracing::warn!(error = %err, delay = ?dur, url = %url, "retrying GET");
        })
        .await
    }

    /// Execute a GET request against a fully-built [`url::Url`].
    ///
    /// Use this when query parameter serialization needs custom handling
    /// (e.g. repeated keys for array values).
    pub(crate) async fn get_url<T: DeserializeOwned>(
        &self,
        url: &url::Url,
    ) -> crate::error::Result<T> {
        let inner = &self.inner;
        let url_str = url.to_string();

        (|| async {
            let resp = inner
                .http
                .get(url.as_str())
                .send()
                .await
                .map_err(LiFiError::Network)?;
            handle_response(resp).await
        })
        .retry(inner.backoff())
        .when(|e: &LiFiError| e.is_retryable())
        .notify(|err: &LiFiError, dur| {
            tracing::warn!(error = %err, delay = ?dur, url = %url_str, "retrying GET");
        })
        .await
    }

    /// Execute a POST request with a JSON body, deserializing the response as `T`.
    pub(crate) async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &(impl serde::Serialize + Sync),
    ) -> crate::error::Result<T> {
        let url = format!("{}{path}", self.inner.config.api_url);
        let json = serde_json::to_value(body)?;
        let inner = &self.inner;

        (|| async {
            let resp = inner
                .http
                .post(&url)
                .json(&json)
                .send()
                .await
                .map_err(LiFiError::Network)?;
            handle_response(resp).await
        })
        .retry(inner.backoff())
        .when(|e: &LiFiError| e.is_retryable())
        .notify(|err: &LiFiError, dur| {
            tracing::warn!(error = %err, delay = ?dur, url = %url, "retrying POST");
        })
        .await
    }
}

/// Parse the `Retry-After` header value as delta-seconds.
fn parse_retry_after(response: &Response) -> Option<Duration> {
    let val = response.headers().get(reqwest::header::RETRY_AFTER)?;
    let secs = val.to_str().ok()?.trim().parse::<u64>().ok()?;
    Some(Duration::from_secs(secs))
}

/// Map an HTTP response to `Ok(T)` or an appropriate [`LiFiError`].
async fn handle_response<T: DeserializeOwned>(response: Response) -> crate::error::Result<T> {
    let status = response.status();

    if status.is_success() {
        let bytes = response.bytes().await.map_err(LiFiError::Network)?;
        let parsed: T = serde_json::from_slice(&bytes)?;
        return Ok(parsed);
    }

    let status_code = status.as_u16();
    let retry_after = parse_retry_after(&response);
    let body = response.text().await.unwrap_or_default();
    let code = http_status_to_error_code(status_code);

    tracing::debug!(status = status_code, ?code, ?retry_after, "API error response");

    Err(LiFiError::Http(HttpErrorDetails {
        status: status_code,
        body,
        code,
        retry_after,
    }))
}
