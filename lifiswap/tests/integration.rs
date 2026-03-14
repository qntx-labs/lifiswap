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
    assert_eq!(resp.status, lifiswap::types::TransferStatus::Done);
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
async fn with_custom_http_client() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/chains"))
        .and(wiremock::matchers::header("x-custom", "hello"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "chains": []
        })))
        .mount(&server)
        .await;

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "x-custom",
        reqwest::header::HeaderValue::from_static("hello"),
    );
    let http = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();

    let config = LiFiConfig::builder()
        .integrator("test")
        .api_url(server.uri())
        .build();

    let client = LiFiClient::with_http_client(config, http);
    let chains = client.get_chains(None).await.expect("request failed");
    assert!(chains.is_empty());
}

#[tokio::test]
async fn get_wallet_balances_success() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/wallets/0x1234567890abcdef/balances"))
        .and(query_param("extended", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "balances": {
                "1": [
                    {
                        "address": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
                        "decimals": 6,
                        "symbol": "USDC",
                        "chainId": 1,
                        "name": "USD Coin",
                        "amount": "1000000",
                        "blockNumber": 19_000_000
                    }
                ],
                "137": []
            }
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let balances = client
        .get_wallet_balances("0x1234567890abcdef")
        .await
        .expect("get_wallet_balances failed");

    assert_eq!(balances.len(), 2);
    let eth_tokens = balances.get(&1).expect("chain 1 missing");
    assert_eq!(eth_tokens.len(), 1);
    assert_eq!(eth_tokens[0].token.symbol, "USDC");
    assert_eq!(eth_tokens[0].amount.as_deref(), Some("1000000"));
}

#[tokio::test]
async fn get_wallet_balances_empty_address_validation() {
    let server = MockServer::start().await;
    let client = test_client(&server.uri());
    let err = client.get_wallet_balances("").await.unwrap_err();

    match &err {
        lifiswap::error::LiFiError::Validation(msg) => {
            assert!(msg.contains("walletAddress"));
        }
        other => panic!("expected Validation error, got: {other:?}"),
    }
}

#[tokio::test]
async fn patch_contract_calls_success() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/patcher"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "target": "0xContractAddress",
                "value": "0",
                "callData": "0xpatched",
                "allowFailure": false,
                "isDelegateCall": false
            }
        ])))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let entries = vec![
        lifiswap::types::PatchCallDataEntry::builder()
            .chain_id(lifiswap::types::ChainId(1))
            .from_token_address("0xTokenAddress")
            .target_contract_address("0xContractAddress")
            .call_data_to_patch("0xoriginal")
            .patches(vec![
                lifiswap::types::CallDataPatch::builder()
                    .amount_to_replace("1000000")
                    .build(),
            ])
            .build(),
    ];

    let result = client
        .patch_contract_calls(&entries)
        .await
        .expect("patch_contract_calls failed");

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].target, "0xContractAddress");
    assert_eq!(result[0].call_data, "0xpatched");
    assert!(!result[0].allow_failure);
}

#[tokio::test]
async fn patch_contract_calls_empty_entries_validation() {
    let server = MockServer::start().await;
    let client = test_client(&server.uri());
    let err = client.patch_contract_calls(&[]).await.unwrap_err();

    match &err {
        lifiswap::error::LiFiError::Validation(msg) => {
            assert!(msg.contains("patch entry"));
        }
        other => panic!("expected Validation error, got: {other:?}"),
    }
}

#[tokio::test]
async fn quote_merges_config_route_options_defaults() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/quote"))
        .and(query_param("slippage", "0.005"))
        .and(query_param("referrer", "0xDefaultReferrer"))
        .and(query_param("allowBridges", "stargate,across"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "step-1",
                "type": "lifi",
                "tool": "stargate",
                "action": {
                    "fromChainId": 1,
                    "toChainId": 137,
                    "fromToken": { "address": "0xUSDC", "decimals": 6, "symbol": "USDC", "chainId": 1, "name": "USDC" },
                    "toToken": { "address": "0xUSDC_POL", "decimals": 6, "symbol": "USDC", "chainId": 137, "name": "USDC" },
                    "fromAmount": "1000000",
                    "fromAddress": "0xSender",
                    "toAddress": "0xSender",
                    "slippage": 0.005
                },
                "estimate": {
                    "fromAmount": "1000000",
                    "toAmount": "990000",
                    "approvalAddress": "0xApproval"
                }
            })),
        )
        .mount(&server)
        .await;

    let client = LiFiClient::new(
        LiFiConfig::builder()
            .integrator("test")
            .api_url(server.uri())
            .retry(RetryConfig::builder().max_retries(0).build())
            .route_options(
                lifiswap::types::RouteOptions::builder()
                    .slippage(0.005)
                    .referrer("0xDefaultReferrer")
                    .bridges(lifiswap::types::ToolFilter {
                        allow: Some(vec!["stargate".into(), "across".into()]),
                        deny: None,
                        prefer: None,
                    })
                    .build(),
            )
            .build(),
    )
    .expect("client");

    let req = lifiswap::types::QuoteRequest::builder()
        .from_chain("1")
        .from_token("0xUSDC")
        .from_address("0xSender")
        .from_amount("1000000")
        .to_chain("137")
        .to_token("0xUSDC_POL")
        .build();

    let _step = client.get_quote(&req).await.expect("get_quote failed");
}

#[tokio::test]
async fn quote_request_overrides_config_defaults() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/quote"))
        .and(query_param("slippage", "0.01"))
        .and(query_param("referrer", "0xOverrideReferrer"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "step-1",
                "type": "lifi",
                "tool": "stargate",
                "action": {
                    "fromChainId": 1,
                    "toChainId": 137,
                    "fromToken": { "address": "0xUSDC", "decimals": 6, "symbol": "USDC", "chainId": 1, "name": "USDC" },
                    "toToken": { "address": "0xUSDC_POL", "decimals": 6, "symbol": "USDC", "chainId": 137, "name": "USDC" },
                    "fromAmount": "1000000",
                    "fromAddress": "0xSender",
                    "toAddress": "0xSender",
                    "slippage": 0.01
                },
                "estimate": {
                    "fromAmount": "1000000",
                    "toAmount": "980000",
                    "approvalAddress": "0xApproval"
                }
            })),
        )
        .mount(&server)
        .await;

    let client = LiFiClient::new(
        LiFiConfig::builder()
            .integrator("test")
            .api_url(server.uri())
            .retry(RetryConfig::builder().max_retries(0).build())
            .route_options(
                lifiswap::types::RouteOptions::builder()
                    .slippage(0.005)
                    .referrer("0xDefaultReferrer")
                    .build(),
            )
            .build(),
    )
    .expect("client");

    let req = lifiswap::types::QuoteRequest::builder()
        .from_chain("1")
        .from_token("0xUSDC")
        .from_address("0xSender")
        .from_amount("1000000")
        .to_chain("137")
        .to_token("0xUSDC_POL")
        .slippage(0.01)
        .referrer("0xOverrideReferrer")
        .build();

    let _step = client.get_quote(&req).await.expect("get_quote failed");
}

#[tokio::test]
async fn api_key_header_is_sent() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/chains"))
        .and(wiremock::matchers::header("x-lifi-api-key", "sk-test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "chains": []
        })))
        .mount(&server)
        .await;

    let client = LiFiClient::new(
        LiFiConfig::builder()
            .integrator("test")
            .api_url(server.uri())
            .api_key("sk-test-key")
            .retry(RetryConfig::builder().max_retries(0).build())
            .build(),
    )
    .expect("client");

    let chains = client.get_chains(None).await.expect("request failed");
    assert!(chains.is_empty());
}

#[tokio::test]
async fn routes_merges_config_route_options() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/advanced/routes"))
        .and(wiremock::matchers::body_partial_json(serde_json::json!({
            "options": {
                "slippage": 0.005
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "routes": []
        })))
        .mount(&server)
        .await;

    let client = LiFiClient::new(
        LiFiConfig::builder()
            .integrator("test")
            .api_url(server.uri())
            .retry(RetryConfig::builder().max_retries(0).build())
            .route_options(
                lifiswap::types::RouteOptions::builder()
                    .slippage(0.005)
                    .build(),
            )
            .build(),
    )
    .expect("client");

    let req = lifiswap::types::RoutesRequest::builder()
        .from_chain_id(lifiswap::types::ChainId(1))
        .to_chain_id(lifiswap::types::ChainId(137))
        .from_token_address("0xUSDC")
        .to_token_address("0xUSDC_POL")
        .from_amount("1000000")
        .build();

    let resp = client.get_routes(&req).await.expect("get_routes failed");
    assert!(resp.routes.is_empty());
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
