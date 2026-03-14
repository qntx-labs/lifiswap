# lifiswap

[![CI][ci-badge]][ci-url]
[![License][license-badge]][license-url]
[![Rust][rust-badge]][rust-url]

[ci-badge]: https://github.com/qntx-labs/lifiswap/actions/workflows/rust.yml/badge.svg
[ci-url]: https://github.com/qntx-labs/lifiswap/actions/workflows/rust.yml
[license-badge]: https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg
[license-url]: LICENSE-MIT
[rust-badge]: https://img.shields.io/badge/rust-edition%202024-orange.svg
[rust-url]: https://doc.rust-lang.org/edition-guide/

**Safe, ergonomic Rust SDK for the [LI.FI](https://li.fi) cross-chain swap and bridge aggregation protocol — any token, any chain, one function call.**

lifiswap wraps the [LI.FI API](https://docs.li.fi) with idiomatic Rust types, providing a high-level `LiFiClient` that handles route discovery, allowance management, transaction signing, cross-chain status polling, and automatic retries. Chain-specific providers (EVM, SVM, BTC, Sui) live in dedicated crates, so you only pay for what you use.

## Crates

| Crate | Description |
| --- | --- |
| **[`lifiswap`](lifiswap/)** | Core SDK — client, types, execution engine |
| **[`lifiswap-evm`](lifiswap-evm/)** | EVM provider via [alloy](https://docs.rs/alloy) (signing, allowance, balance) |
| **[`lifiswap-svm`](lifiswap-svm/)** | Solana provider (planned) |
| **[`lifiswap-btc`](lifiswap-btc/)** | Bitcoin provider (planned) |
| **[`lifiswap-sui`](lifiswap-sui/)** | Sui provider (planned) |
| **[`lifiswap-cli`](lifiswap-cli/)** | CLI tool (planned) |

## Quick Start

### Install the CLI

**Shell** (macOS / Linux):

```sh
curl -fsSL https://sh.qntx.fun/lifiswap | sh
```

**PowerShell** (Windows):

```powershell
irm https://sh.qntx.fun/lifiswap/ps | iex
```

### One-Line Swap

The simplest way to perform a cross-chain swap — one method call does everything: fetch the optimal quote from LI.FI's smart routing API, convert it to a route, check balances, approve tokens, sign transactions, and poll for completion.

```rust
use lifiswap::{LiFiClient, LiFiConfig};
use lifiswap::types::QuoteRequest;
use lifiswap_evm::EvmProvider;
use alloy::signers::local::PrivateKeySigner;

#[tokio::main]
async fn main() -> lifiswap::error::Result<()> {
    // Create client
    let client = LiFiClient::new(
        LiFiConfig::builder().integrator("my-app").build(),
    )?;

    // Register chain provider
    let signer: PrivateKeySigner = "0xac0974...".parse().expect("valid key");
    client.add_provider(EvmProvider::new(signer, "https://eth.llamarpc.com"));

    // Swap — that's it
    let result = client
        .swap(
            &QuoteRequest::builder()
                .from_chain("42161")                                      // Arbitrum
                .from_token("0xaf88d065e77c8cC2239327C5EDb3A432268e5831") // USDC
                .from_address("0xYourWallet")
                .from_amount("10000000")                                  // 10 USDC
                .to_chain("10")                                           // Optimism
                .to_token("0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1")   // DAI
                .build(),
            Default::default(),
        )
        .await?;

    eprintln!("done: route {}", result.id);
    Ok(())
}
```

### Step-by-Step Control

For more control, break the flow into individual steps:

```rust
// Get a quote, then execute it
let quote = client.get_quote(&request).await?;
let result = client.execute_quote(quote, Default::default()).await?;

// Or: get multiple routes, pick one, then execute
let routes = client.get_routes(&routes_request).await?;
let best = routes.routes.into_iter().next().expect("at least one route");
let result = client.execute_route(best, Default::default()).await?;
```

### Query-Only Usage

No providers needed for read-only API calls:

```rust
use lifiswap::{LiFiClient, LiFiConfig};

let client = LiFiClient::new(
    LiFiConfig::builder().integrator("my-app").build(),
)?;

let chains = client.get_chains(None).await?;
let tokens = client.get_tokens(None).await?;
let tools = client.get_tools(None).await?;
```

## Architecture

- **lifiswap** — Core SDK. `LiFiClient` is built via `LiFiConfig` builder with optional API key, retry config, and custom HTTP client. All 18 LI.FI API endpoints are covered. The execution engine handles the full lifecycle: balance checks → allowance approval → transaction signing → cross-chain status polling → retry on failure. `LiFiClient` is `Clone + Send + Sync` (`Arc<Inner>`).
- **lifiswap-evm** — EVM chain provider using [alloy](https://docs.rs/alloy). Handles ERC-20 balance queries, token approval (infinite approve), transaction signing via `EthereumWallet`, and receipt confirmation. Implements the `Provider` and `StepExecutor` traits.
- **lifiswap-svm / lifiswap-btc / lifiswap-sui** — Chain-specific providers for Solana, Bitcoin, and Sui (scaffolded, implementations planned).
- **lifiswap-cli** — Command-line interface for cross-chain swaps (planned).

## Configuration

```rust
use std::time::Duration;
use lifiswap::{LiFiClient, LiFiConfig, RetryConfig};

let client = LiFiClient::new(
    LiFiConfig::builder()
        .integrator("my-app")
        .api_key("lifi-...")
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

// Or inject a custom reqwest::Client
let http = reqwest::Client::builder()
    .proxy(reqwest::Proxy::all("http://proxy:8080")?)
    .build()?;
let client = LiFiClient::with_http_client(
    LiFiConfig::builder().integrator("my-app").build(),
    http,
);
```

## API Coverage

All [LI.FI REST API](https://docs.li.fi/api-reference/introduction) endpoints are supported:

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

## Execution Lifecycle

The execution engine (`client.swap()` / `client.execute_route()`) automates the complete cross-chain transfer flow:

```text
Quote → Route → [for each step]:
  ├── Check Balance    — verify sufficient token balance
  ├── Check Allowance  — query current ERC-20 allowance
  ├── Set Allowance    — approve spender if insufficient (ERC-20 only)
  ├── Prepare Tx       — fetch transaction data from LI.FI API
  ├── Sign & Send      — sign with wallet, submit to chain
  └── Wait for Status  — poll LI.FI status API until DONE/FAILED
```

Failed steps can be resumed with `client.resume_route()`. Active routes can be stopped with `client.stop_route_execution()` and listed with `client.get_active_routes()`.

## Security

This library has **not** been independently audited. See [SECURITY.md](SECURITY.md) for full disclaimer, supported versions, and vulnerability reporting instructions.

- No key material is logged or persisted by the SDK
- All HTTP communication uses TLS (rustls by default, native-tls optional)
- API keys are sent via dedicated header, never in query strings
- `LiFiClient` is thread-safe; providers are behind `Arc<RwLock<>>`

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
