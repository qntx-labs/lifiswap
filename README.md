# LI.FI Swap

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

| Crate | | Description |
| --- | --- | --- |
| **[`lifiswap`](lifiswap/)** | [![crates.io][lifiswap-crate]][lifiswap-crate-url] [![docs.rs][lifiswap-doc]][lifiswap-doc-url] | Core SDK — client, types, execution engine |
| **[`lifiswap-evm`](lifiswap-evm/)** | [![crates.io][evm-crate]][evm-crate-url] [![docs.rs][evm-doc]][evm-doc-url] | EVM provider via [alloy](https://docs.rs/alloy) v1 |
| **[`lifiswap-svm`](lifiswap-svm/)** | [![crates.io][svm-crate]][svm-crate-url] [![docs.rs][svm-doc]][svm-doc-url] | Solana provider via [solana-sdk](https://docs.rs/solana-sdk) v3 |
| **[`lifiswap-btc`](lifiswap-btc/)** | [![crates.io][btc-crate]][btc-crate-url] [![docs.rs][btc-doc]][btc-doc-url] | Bitcoin provider via [bitcoin](https://docs.rs/bitcoin) v0.32 |
| **[`lifiswap-sui`](lifiswap-sui/)** | [![crates.io][sui-crate]][sui-crate-url] [![docs.rs][sui-doc]][sui-doc-url] | Sui provider (planned) |
| **[`lifiswap-cli`](lifiswap-cli/)** | [![crates.io][cli-crate]][cli-crate-url] | CLI tool (planned) |

[lifiswap-crate]: https://img.shields.io/crates/v/lifiswap.svg
[lifiswap-crate-url]: https://crates.io/crates/lifiswap
[lifiswap-doc]: https://img.shields.io/docsrs/lifiswap.svg
[lifiswap-doc-url]: https://docs.rs/lifiswap
[evm-crate]: https://img.shields.io/crates/v/lifiswap-evm.svg
[evm-crate-url]: https://crates.io/crates/lifiswap-evm
[evm-doc]: https://img.shields.io/docsrs/lifiswap-evm.svg
[evm-doc-url]: https://docs.rs/lifiswap-evm
[svm-crate]: https://img.shields.io/crates/v/lifiswap-svm.svg
[svm-crate-url]: https://crates.io/crates/lifiswap-svm
[svm-doc]: https://img.shields.io/docsrs/lifiswap-svm.svg
[svm-doc-url]: https://docs.rs/lifiswap-svm
[btc-crate]: https://img.shields.io/crates/v/lifiswap-btc.svg
[btc-crate-url]: https://crates.io/crates/lifiswap-btc
[btc-doc]: https://img.shields.io/docsrs/lifiswap-btc.svg
[btc-doc-url]: https://docs.rs/lifiswap-btc
[sui-crate]: https://img.shields.io/crates/v/lifiswap-sui.svg
[sui-crate-url]: https://crates.io/crates/lifiswap-sui
[sui-doc]: https://img.shields.io/docsrs/lifiswap-sui.svg
[sui-doc-url]: https://docs.rs/lifiswap-sui
[cli-crate]: https://img.shields.io/crates/v/lifiswap-cli.svg
[cli-crate-url]: https://crates.io/crates/lifiswap-cli

## Quick Start

### Install the CLI

**Shell** (macOS / Linux):

```sh
curl -fsSL https://sh.qntx.fun/labs/lifiswap | sh
```

**PowerShell** (Windows):

```powershell
irm https://sh.qntx.fun/labs/lifiswap/ps | iex
```

### One-Line Swap (EVM)

```rust
use lifiswap::{LiFiClient, LiFiConfig};
use lifiswap::types::{ExecutionOptions, QuoteRequest};
use lifiswap_evm::{EvmProvider, LocalSigner};

let client = LiFiClient::new(LiFiConfig::builder().integrator("my-app").build())?;
client.add_provider(EvmProvider::new(LocalSigner::new(key, rpc.clone()), rpc));

let result = client.swap(
    &QuoteRequest::builder()
        .from_chain("42161").from_token(USDC_ARB)
        .from_address(&wallet).from_amount("1000000")
        .to_chain("8453").to_token(USDC_BASE)
        .build(),
    ExecutionOptions::default(),
).await?;
```

### Multi-Chain Providers

```rust
use lifiswap_evm::{EvmProvider, LocalSigner};
use lifiswap_svm::{SvmProvider, KeypairSigner as SvmKeypairSigner};
use lifiswap_btc::{BtcProvider, KeypairSigner as BtcKeypairSigner};

let client = LiFiClient::new(LiFiConfig::builder().integrator("my-app").build())?;

// EVM (Ethereum, Arbitrum, Base, …)
client.add_provider(EvmProvider::new(LocalSigner::new(evm_key, rpc_url), rpc_url));

// Solana
let svm_signer = SvmKeypairSigner::new(solana_keypair);
client.add_provider(SvmProvider::new(svm_signer, &solana_rpc));

// Bitcoin
let btc_signer = BtcKeypairSigner::new(btc_private_key, bitcoin::Network::Bitcoin);
client.add_provider(BtcProvider::new(btc_signer));
```

### Step-by-Step Control

```rust
let quote = client.get_quote(&request).await?;
let result = client.execute_quote(quote, Default::default()).await?;

let routes = client.get_routes(&routes_request).await?;
let best = routes.routes.into_iter().next().expect("at least one route");
let result = client.execute_route(best, Default::default()).await?;
```

### Query-Only Usage

```rust
let client = LiFiClient::new(LiFiConfig::builder().integrator("my-app").build())?;
let chains = client.get_chains(None).await?;
let tokens = client.get_tokens(None).await?;
```

> See [`examples/`](lifiswap-evm/examples/) for complete runnable demos:
> [`swap`](lifiswap-evm/examples/swap.rs) ·
> [`cross_chain_usdc`](lifiswap-evm/examples/cross_chain_usdc.rs) ·
> [`compare_routes`](lifiswap-evm/examples/compare_routes.rs) ·
> [`query_only`](lifiswap-evm/examples/query_only.rs)

## Architecture

### Core (`lifiswap`)

`LiFiClient` is the single entry point — `Clone + Send + Sync` via `Arc<ClientInner>`. Configuration uses compile-time [`bon::Builder`](https://docs.rs/bon) for `LiFiConfig` and `RetryConfig`. HTTP requests retry automatically with exponential backoff via [`backon`](https://docs.rs/backon) (retryable on 429/5xx, timeouts, and connection errors).

The execution engine orchestrates cross-chain routes through two core traits:

- **`Provider`** — chain-specific operations: address validation, name resolution, balance queries, and `StepExecutor` creation.
- **`StepExecutor`** — executes a single step via `TaskPipeline`, a sequential chain of `ExecutionTask` implementations.

Shared step execution logic lives in `run_step_pipeline()`, which every chain executor delegates to. This handles `StatusManager` lifecycle, `ExecutionContext` construction, pipeline execution, and error recovery — matching the TypeScript SDK's `BaseStepExecutor.executeStep` pattern.

### EVM (`lifiswap-evm`)

Built on [alloy](https://docs.rs/alloy) v1. The `EvmSigner` trait abstracts signing backends (private key, hardware wallet, browser extension, EIP-5792 batching).

| Feature | Details |
| --- | --- |
| **Signing** | `EvmSigner` trait with `LocalSigner` implementation (EIP-1559, EIP-712) |
| **Allowance** | Auto-detect, reset (USDT-style), and infinite approve |
| **Permit2** | Gasless approvals via Uniswap Permit2 + LI.FI Permit2Proxy |
| **Relay** | Gasless execution — sign EIP-712 typed data, submit via relayer API |
| **Batching** | EIP-5792 `wallet_sendCalls` for atomic approve+swap |
| **ENS** | Name resolution via `alloy-ens` `ProviderEnsExt::resolve_name()` |
| **Multi-chain RPC** | `RpcUrlResolver` trait for per-chain RPC endpoint selection |
| **Receipt** | `confirm_transaction()` on signer, no separate provider needed |

**Task pipeline:** CheckPermits → CheckBalance → Allowance → PrepareTransaction → Sign&Execute (or Relay, or Batched) → WaitForReceipt → WaitForTransactionStatus

### Solana (`lifiswap-svm`)

Built on [solana-sdk](https://docs.rs/solana-sdk) v3. The `SvmSigner` trait abstracts keypair signing.

| Feature | Details |
| --- | --- |
| **Signing** | `SvmSigner` trait with `KeypairSigner` implementation |
| **Balances** | Native SOL + SPL Token + Token-2022 via ATA derivation |
| **RPC pool** | `RpcPool` with sequential retry across multiple endpoints |
| **Simulation** | Pre-send simulation with configurable skip |
| **SNS** | Solana Name Service `.sol` domain resolution |
| **Confirmation** | Send-and-confirm with automatic resend on blockhash expiry |

**Task pipeline:** CheckBalance → PrepareTransaction → Sign → SendAndConfirm → WaitForTransactionStatus

### Bitcoin (`lifiswap-btc`)

Built on [bitcoin](https://docs.rs/bitcoin) v0.32. The `BtcSigner` trait abstracts PSBT signing.

| Feature | Details |
| --- | --- |
| **Signing** | `BtcSigner` trait with `KeypairSigner` (BIP-174 PSBT) |
| **PSBT finalize** | Manual finalization: P2WPKH, P2TR key-path, P2SH-P2WPKH |
| **Blockchain API** | `BlockchainApi` — mempool.space REST with multi-backend fallback |
| **Confirmation** | Polling-based (10s interval, 30min timeout) |

**Task pipeline:** CheckBalance → PrepareTransaction → Sign&Broadcast → Confirm → WaitForTransactionStatus

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

**Defaults:** API URL `https://li.quest/v1` · Timeout 30s · Retry 3× (300ms–10s, jitter) · TLS via rustls (feature `native-tls` available)

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
  ├─ Provider::create_step_executor()
  │    └─ TaskPipeline::run()
  │         ├── Check Balance     — verify sufficient token balance
  │         ├── Check Allowance   — query current spender allowance (EVM)
  │         ├── Set Allowance     — approve spender if needed (EVM)
  │         ├── Prepare Tx        — fetch transaction data from LI.FI API
  │         ├── Sign & Send       — sign with signer, broadcast to network
  │         └── Wait for Status   — poll LI.FI status API until DONE/FAILED
  └─ StatusManager updates execution state in real-time
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
