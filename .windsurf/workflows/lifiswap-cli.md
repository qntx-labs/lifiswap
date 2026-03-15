---
description: How to develop, extend, and maintain the lifiswap-cli crate — the command-line interface for LiFi cross-chain swap SDK. Use this skill whenever working on CLI commands, adding new subcommands, fixing CLI bugs, improving CLI output formatting, or integrating new chain providers into the CLI. Also use when the user mentions "cli", "command line", "lifiswap swap", "lifiswap quote", or any terminal-based interaction with the lifiswap SDK.
---

# lifiswap-cli Development Guide

## Overview

`lifiswap-cli` is a professional command-line tool for cross-chain and same-chain token swaps powered by the LiFi protocol. It wraps the `lifiswap` Rust SDK and its chain provider crates (`lifiswap-evm`, `lifiswap-svm`, `lifiswap-btc`) into an ergonomic CLI experience with colored output, progress indicators, and multiple output formats.

Binary name: `lifiswap`

## Architecture

```
lifiswap-cli/src/
├── main.rs              # Entry point: parse args, init tracing, dispatch
├── app.rs               # App context: LiFiClient + providers + output config
├── output.rs            # Output formatting: table / json / compact
├── wallet.rs            # Wallet/signer loading from env vars or keyfiles
├── progress.rs          # Progress bars and status spinners
├── commands/
│   ├── mod.rs           # Command enum (clap subcommands)
│   ├── chains.rs        # `lifiswap chains` — list supported chains
│   ├── tokens.rs        # `lifiswap tokens` — search/list tokens
│   ├── tools.rs         # `lifiswap tools` — list bridges & exchanges
│   ├── connections.rs   # `lifiswap connections` — chain connectivity
│   ├── gas.rs           # `lifiswap gas` — gas recommendation
│   ├── status.rs        # `lifiswap status` — check tx status
│   ├── quote.rs         # `lifiswap quote` — get swap/bridge quote
│   ├── routes.rs        # `lifiswap routes` — compare multiple routes
│   ├── balances.rs      # `lifiswap balances` — wallet token balances
│   └── swap.rs          # `lifiswap swap` — execute a swap end-to-end
```

## Technology Stack

### Required Dependencies (Cargo.toml)

```toml
[dependencies]
lifiswap = { workspace = true }
lifiswap-evm = { workspace = true }
lifiswap-svm = { workspace = true }
lifiswap-btc = { workspace = true }

# CLI framework
clap = { version = "4", features = ["derive", "env", "color"] }

# Async runtime
tokio = { workspace = true }

# Output & UX
comfy-table = "7"
console = "0.15"
indicatif = "0.17"
dialoguer = "0.11"

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }

# Logging
tracing = { workspace = true }
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Environment
dotenvy = "0.15"

# Error handling
anyhow = "1"

# Chain-specific signing deps
alloy = { workspace = true }
solana-sdk = { workspace = true }
bitcoin = { workspace = true }
url = { workspace = true }
```

### Design Principles

1. **Query commands need no wallet** — `chains`, `tokens`, `tools`, `connections`, `gas`, `status`, `quote`, `routes`, `balances` work with just an integrator name
2. **Execution commands need a wallet** — `swap` requires a private key
3. **Multiple output formats** — `--output table` (default, human-friendly), `--output json` (machine-readable), `--output compact` (minimal)
4. **Environment variable fallbacks** — all config via `LIFI_INTEGRATOR`, `LIFI_API_KEY`, `LIFI_API_URL`, `LIFI_PRIVATE_KEY`, `LIFI_RPC_URL`
5. **`.env` file support** — auto-load `.env` from CWD via `dotenvy`
6. **Colored output** — use `console` crate styles, respect `--color` flag and `NO_COLOR` env
7. **Progress indicators** — spinners for API calls, progress bars for swap execution
8. **Confirmation prompts** — interactive confirmation before executing swaps (skip with `--yes`)

## Global CLI Structure (clap derive)

```rust
#[derive(Parser)]
#[command(name = "lifiswap", version, about = "Cross-chain swap CLI powered by LiFi")]
struct Cli {
    /// Integrator name
    #[arg(long, env = "LIFI_INTEGRATOR", default_value = "lifiswap-cli")]
    integrator: String,

    /// API key for authenticated endpoints
    #[arg(long, env = "LIFI_API_KEY")]
    api_key: Option<String>,

    /// API base URL
    #[arg(long, env = "LIFI_API_URL")]
    api_url: Option<String>,

    /// Output format
    #[arg(long, default_value = "table", value_parser = ["table", "json", "compact"])]
    output: String,

    /// Color mode
    #[arg(long, default_value = "auto", value_parser = ["auto", "always", "never"])]
    color: String,

    /// Verbose logging (repeat for more: -v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}
```

## Command Reference

### Query Commands

#### `lifiswap chains`
Lists supported chains with optional type filter.
```
lifiswap chains [--type evm|svm|utxo|mvm]
```
Output: chain ID, name, type, native token, key bool flags.

#### `lifiswap tokens`
Search and list tokens on a specific chain.
```
lifiswap tokens --chain <CHAIN_ID> [--search <QUERY>]
```
Output: address, symbol, name, decimals, price USD.

#### `lifiswap tools`
List available bridges and exchanges.
```
lifiswap tools
```
Output: name, type (bridge/exchange), supported chains count.

#### `lifiswap connections`
Show connections between chains.
```
lifiswap connections --from-chain <ID> --to-chain <ID> [--from-token <ADDR>] [--to-token <ADDR>]
```

#### `lifiswap gas`
Get gas recommendation for a chain.
```
lifiswap gas --chain <CHAIN_ID>
```

#### `lifiswap status`
Check transaction execution status.
```
lifiswap status --tx-hash <HASH> [--bridge <TOOL>] [--from-chain <ID>] [--to-chain <ID>]
```

#### `lifiswap quote`
Get a single best quote for a swap/bridge.
```
lifiswap quote \
  --from-chain <ID> --from-token <ADDR> --from-amount <WEI> \
  --to-chain <ID> --to-token <ADDR> \
  --from-address <WALLET>
```
Output: tool, estimated output, fees, duration, gas cost.

#### `lifiswap routes`
Compare multiple routes (like the compare_routes example).
```
lifiswap routes \
  --from-chain <ID> --from-token <ADDR> --from-amount <WEI> \
  --to-chain <ID> --to-token <ADDR> \
  --from-address <WALLET>
```
Output: ranked table of routes with amounts, fees, tools, duration.

#### `lifiswap balances`
Get token balances for a wallet address.
```
lifiswap balances --address <WALLET> [--chains <ID,ID,...>]
```

### Execution Commands

#### `lifiswap swap`
Execute a cross-chain or same-chain swap end-to-end.
```
lifiswap swap \
  --from-chain <ID> --from-token <ADDR> --from-amount <WEI> \
  --to-chain <ID> --to-token <ADDR> \
  [--private-key <KEY>] [--rpc-url <URL>] \
  [--slippage <PERCENT>] \
  [--yes]  # skip confirmation prompt
```
Environment: `LIFI_PRIVATE_KEY`, `LIFI_RPC_URL`

Flow:
1. Load wallet from `--private-key` or `LIFI_PRIVATE_KEY`
2. Detect chain type from `--from-chain` → register appropriate provider
3. Fetch quote with spinner
4. Display quote summary (from/to amounts, fees, route)
5. Prompt for confirmation (unless `--yes`)
6. Execute with live progress updates
7. Display final result (tx hash, explorer link, amounts)

## Implementation Patterns

### App Context Pattern

Every command receives an `App` struct containing shared state:

```rust
pub struct App {
    pub client: LiFiClient,
    pub output: OutputFormat,
    pub color: ColorChoice,
}
```

The `App` is created in `main.rs` after parsing global args, then passed to each command handler.

### Output Formatting Pattern

Use a trait-based approach for consistent output:

```rust
pub enum OutputFormat {
    Table,
    Json,
    Compact,
}

impl App {
    pub fn print_table(&self, table: comfy_table::Table) { ... }
    pub fn print_json<T: Serialize>(&self, value: &T) { ... }
    pub fn print_value(&self, label: &str, value: &str) { ... }
}
```

For `--output json`, every command serializes its result as JSON to stdout. For `--output table`, use `comfy-table` with styled headers. This makes the CLI both human-friendly and scriptable.

### Wallet Loading Pattern

Detect chain type and load the appropriate signer:

```rust
pub enum WalletConfig {
    Evm { private_key: String, rpc_url: url::Url },
    Svm { private_key: String, rpc_urls: Vec<url::Url> },
    Btc { wif: String, network: bitcoin::Network },
}
```

The chain type is inferred from the `--from-chain` ID by querying the LiFi chains API. Then the correct provider crate's `KeypairSigner` is instantiated.

### Progress Pattern

Wrap long API calls with `indicatif` spinners:

```rust
let spinner = ProgressBar::new_spinner();
spinner.set_message("Fetching quote...");
spinner.enable_steady_tick(Duration::from_millis(100));
let quote = client.get_quote(&request).await?;
spinner.finish_with_message("Quote received");
```

For swap execution, use the `update_route_hook` to drive a multi-step progress bar:

```rust
let progress = MultiProgress::new();
// Create bars for each step, update via hook
```

### Error Display Pattern

Use `anyhow` for top-level error handling. Display user-friendly errors:

```rust
fn main() {
    if let Err(e) = run() {
        let style = console::Style::new().red().bold();
        eprintln!("{} {e:#}", style.apply_to("error:"));
        std::process::exit(1);
    }
}
```

## Testing Strategy

- **Unit tests**: Each command module has tests with mocked API responses
- **Integration tests**: Test full command execution with `assert_cmd` crate
- **Snapshot tests**: Use `insta` for output format regression testing
- **Manual testing**: The query commands can be tested against live LiFi API without a wallet

## Adding a New Command

1. Create `commands/<name>.rs` with a clap Args struct and async `run(app: &App, args: Args)` function
2. Add variant to `Commands` enum in `commands/mod.rs`
3. Add dispatch in `main.rs` match block
4. Implement table + JSON output paths
5. Add tests

## Adding a New Chain Provider

When `lifiswap-sui` is implemented:
1. Add `lifiswap-sui` dependency to Cargo.toml
2. Extend `WalletConfig` enum with `Sui { ... }` variant
3. Extend `register_provider()` in `wallet.rs` to handle `ChainType::MVM`
4. The rest of the CLI works automatically (quote, swap, status all go through the SDK)

## Code Quality Requirements

- Zero clippy warnings: `cargo clippy -- -W clippy::all`
- Consistent error messages: sentence case, no trailing period for short messages
- All public items documented with rustdoc
- No `unwrap()` in production code — use `anyhow::Context` for error context
- Follow existing workspace patterns: `bon::Builder`, `tracing` for logging
