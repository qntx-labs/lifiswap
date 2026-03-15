#![allow(clippy::print_stdout)]
//! One-line cross-chain swap.
//!
//! The simplest possible usage: `client.swap()` handles everything —
//! quote fetching, route conversion, balance check, token approval,
//! transaction signing, and cross-chain status polling.
//!
//! ```bash
//! PRIVATE_KEY=0xac0974... cargo run --example swap -p lifiswap-evm
//! ```

use lifiswap::types::{ExecutionOptions, QuoteRequest};
use lifiswap::{LiFiClient, LiFiConfig};
use lifiswap_evm::{EvmProvider, LocalSigner};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let private_key = std::env::var("PRIVATE_KEY").expect("PRIVATE_KEY env var required");
    let key: alloy::signers::local::PrivateKeySigner = private_key.parse()?;
    let wallet = format!("{:#x}", key.address());
    let rpc: url::Url = "https://arb1.arbitrum.io/rpc".parse()?;

    let client = LiFiClient::new(LiFiConfig::builder().integrator("lifiswap-example").build())?;
    client.add_provider(EvmProvider::new(LocalSigner::new(key, rpc.clone()), rpc));

    let result = client
        .swap(
            &QuoteRequest::builder()
                .from_chain("42161")                                      // Arbitrum
                .from_token("0xaf88d065e77c8cC2239327C5EDb3A432268e5831") // USDC
                .from_address(&wallet)
                .from_amount("1000000")                                   // 1 USDC
                .to_chain("8453")                                         // Base
                .to_token("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913")   // USDC
                .build(),
            ExecutionOptions::default(),
        )
        .await?;

    println!("Done! Route {} completed.", result.id);
    Ok(())
}
