//! Integration tests for the `LiFi` SDK using wiremock.
#![allow(clippy::panic)]

use std::time::Duration;

use lifiswap::LiFiClient;
use lifiswap::client::{LiFiConfig, RetryConfig};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Helper: create a client pointing at the given mock server.
fn test_client(base_url: &str) -> LiFiClient {
    LiFiClient::new(
        LiFiConfig::builder()
            .integrator("test-integrator")
            .api_url(base_url)
            .retry(
                RetryConfig::builder()
                    .max_retries(2)
                    .min_delay(Duration::from_millis(50))
                    .max_delay(Duration::from_millis(200))
                    .build(),
            )
            .timeout(Duration::from_secs(5))
            .build(),
    )
    .expect("failed to create test client")
}

#[tokio::test]
async fn get_chains_success() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/chains"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "chains": [
                {
                    "key": "eth",
                    "chainType": "EVM",
                    "id": 1,
                    "name": "Ethereum",
                    "coin": "ETH",
                    "mainnet": true,
                    "metamask": {
                        "chainId": "0x1",
                        "chainName": "Ethereum Mainnet",
                        "nativeCurrency": { "name": "ETH", "symbol": "ETH", "decimals": 18 },
                        "rpcUrls": ["https://rpc.ankr.com/eth"],
                        "blockExplorerUrls": ["https://etherscan.io"]
                    }
                }
            ]
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let chains = client.get_chains(None).await.expect("get_chains failed");
    assert_eq!(chains.len(), 1);
    assert_eq!(chains[0].name, "Ethereum");
}

#[tokio::test]
async fn get_chains_with_filter() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/chains"))
        .and(query_param("chainTypes", "EVM"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "chains": []
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let req = lifiswap::types::ChainsRequest::builder()
        .chain_types(vec![lifiswap::types::ChainType::EVM])
        .build();
    let chains = client
        .get_chains(Some(&req))
        .await
        .expect("get_chains failed");
    assert!(chains.is_empty());
}

#[tokio::test]
async fn http_error_maps_to_lifi_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/chains"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "message": "Bad Request"
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let err = client.get_chains(None).await.unwrap_err();

    match &err {
        lifiswap::error::LiFiError::Http(details) => {
            assert_eq!(details.status, 400);
            assert_eq!(
                details.code,
                lifiswap::error::LiFiErrorCode::ValidationError
            );
        }
        other => panic!("expected Http error, got: {other:?}"),
    }
}

#[tokio::test]
async fn retries_on_server_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/chains"))
        .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
        .expect(3) // 1 initial + 2 retries
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let err = client.get_chains(None).await.unwrap_err();

    match &err {
        lifiswap::error::LiFiError::Http(details) => {
            assert_eq!(details.status, 500);
        }
        other => panic!("expected Http error, got: {other:?}"),
    }
}

#[tokio::test]
async fn retries_on_429_with_retry_after() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/chains"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "1")
                .set_body_string("rate limited"),
        )
        .expect(3) // 1 initial + 2 retries
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let err = client.get_chains(None).await.unwrap_err();

    match &err {
        lifiswap::error::LiFiError::Http(details) => {
            assert_eq!(details.status, 429);
            assert_eq!(details.retry_after, Some(Duration::from_secs(1)));
        }
        other => panic!("expected Http error, got: {other:?}"),
    }
}

#[tokio::test]
async fn no_retry_on_client_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/chains"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .expect(1) // no retries
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let err = client.get_chains(None).await.unwrap_err();

    match &err {
        lifiswap::error::LiFiError::Http(details) => {
            assert_eq!(details.status, 404);
        }
        other => panic!("expected Http error, got: {other:?}"),
    }
}

#[tokio::test]
async fn post_routes_success() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/advanced/routes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "routes": []
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let req = lifiswap::types::RoutesRequest::builder()
        .from_chain_id(lifiswap::types::ChainId(1))
        .to_chain_id(lifiswap::types::ChainId(137))
        .from_token_address("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
        .to_token_address("0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174")
        .from_amount("1000000")
        .build();

    let resp = client.get_routes(&req).await.expect("get_routes failed");
    assert!(resp.routes.is_empty());
}

#[tokio::test]
async fn get_status_success() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/status"))
        .and(query_param("txHash", "0xabc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "transactionId": "0xabc123",
            "sending": {},
            "receiving": {},
            "tool": "stargate",
            "status": "DONE",
            "substatus": "COMPLETED",
            "substatusMessage": "Transfer complete"
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let req = lifiswap::types::StatusRequest::builder()
        .tx_hash("0xabc123")
        .build();

    let resp = client.get_status(&req).await.expect("get_status failed");
    assert_eq!(resp.status, "DONE");
}

#[tokio::test]
async fn client_sends_sdk_headers() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/chains"))
        .and(wiremock::matchers::header(
            "x-lifi-integrator",
            "test-integrator",
        ))
        .and(wiremock::matchers::header_exists("x-lifi-sdk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "chains": []
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let chains = client.get_chains(None).await.expect("get_chains failed");
    assert!(chains.is_empty());
}

#[tokio::test]
async fn client_is_clone_and_send() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/chains"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "chains": []
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let client2 = client.clone();

    let handle = tokio::spawn(async move { client2.get_chains(None).await });

    let _ = handle.await.unwrap().unwrap();
}
