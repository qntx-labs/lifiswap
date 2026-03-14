# lifiswap

Rust SDK for the [LI.FI](https://li.fi) cross-chain swap and bridge aggregation API.

## Features

- **Type-safe builders** — all request types use [`bon::Builder`](https://docs.rs/bon) with compile-time checks
- **Automatic retries** — exponential backoff with jitter via [`backon`](https://docs.rs/backon), configurable per-client
- **`Retry-After` support** — 429 responses parse the header and expose it on errors
- **Thread-safe** — `LiFiClient` is `Clone + Send + Sync` (`Arc<Inner>`)
- **Custom HTTP client** — inject your own `reqwest::Client` for proxies, middleware, or custom TLS
- **Structured tracing** — retry warnings and error details via [`tracing`](https://docs.rs/tracing)

## Quick Start

```rust
use lifiswap::{LiFiClient, LiFiConfig};

#[tokio::main]
async fn main() -> lifiswap::error::Result<()> {
    let client = LiFiClient::new(
        LiFiConfig::builder().integrator("my-app").build(),
    )?;

    let chains = client.get_chains(None).await?;
    println!("supported chains: {}", chains.len());
    Ok(())
}
```

## Builder Pattern

All request types support ergonomic builders:

```rust
use lifiswap::types::{QuoteRequest, StatusRequest, RoutesRequest, ChainId};

let quote_req = QuoteRequest::builder()
    .from_chain("1")
    .from_token("0xUSDC...")
    .from_address("0xYourWallet...")
    .from_amount("1000000")
    .to_chain("137")
    .to_token("0xUSDC_POL...")
    .build();

let status_req = StatusRequest::builder()
    .tx_hash("0xabc123...")
    .build();

let routes_req = RoutesRequest::builder()
    .from_chain_id(ChainId(1))
    .to_chain_id(ChainId(137))
    .from_token_address("0xUSDC...")
    .to_token_address("0xUSDC_POL...")
    .from_amount("1000000")
    .build();
```

## Configuration

```rust
use std::time::Duration;
use lifiswap::{LiFiClient, LiFiConfig};
use lifiswap::client::RetryConfig;

let client = LiFiClient::new(
    LiFiConfig::builder()
        .integrator("my-app")
        .api_key("sk-...")
        .retry(
            RetryConfig::builder()
                .max_retries(5)
                .min_delay(Duration::from_millis(500))
                .max_delay(Duration::from_secs(30))
                .build(),
        )
        .timeout(Duration::from_secs(60))
        .build(),
)?;
```

## Custom HTTP Client

Inject a pre-configured `reqwest::Client` for proxies, custom TLS, or middleware:

```rust
let http = reqwest::Client::builder()
    .proxy(reqwest::Proxy::all("http://proxy:8080")?)
    .build()?;

let client = LiFiClient::with_http_client(
    LiFiConfig::builder().integrator("my-app").build(),
    http,
);
```

## API Coverage

| Endpoint | Method |
| --- | --- |
| `GET /chains` | `get_chains()` |
| `GET /connections` | `get_connections()` |
| `GET /tokens` | `get_tokens()` |
| `GET /token` | `get_token()` |
| `GET /tools` | `get_tools()` |
| `GET /quote` | `get_quote()` |
| `GET /quote/toAmount` | `get_quote_to_amount()` |
| `POST /quote/contractCalls` | `get_contract_calls_quote()` |
| `POST /advanced/routes` | `get_routes()` |
| `POST /advanced/stepTransaction` | `get_step_transaction()` |
| `GET /status` | `get_status()` |
| `GET /gas/suggestion/{chainId}` | `get_gas_recommendation()` |
| `GET /relayer/quote` | `get_relayer_quote()` |
| `POST /advanced/relay` | `relay_transaction()` |
| `GET /relayer/status` | `get_relayed_transaction_status()` |
| `GET /analytics/transfers` | `get_transaction_history()` |
| `GET /wallets/{addr}/balances` | `get_wallet_balances()` |
| `POST /patcher` | `patch_contract_calls()` |

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this project shall be dual-licensed as above, without any additional terms or conditions.

---

<div align="center">

A **[QNTX](https://qntx.fun)** open-source project.

<a href="https://qntx.fun"><img alt="QNTX" width="369" src="https://raw.githubusercontent.com/qntx/.github/main/profile/qntx-banner.svg" /></a>

<!--prettier-ignore-->
Code is law. We write both.

</div>
