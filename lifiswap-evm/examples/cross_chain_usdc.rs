#![allow(clippy::print_stdout, clippy::collapsible_if)]
//! Minimal cross-chain USDC swap example.
//!
//! Bridges USDC from Monad → Base using a local private key signer.
//!
//! # Prerequisites
//!
//! 1. Have USDC on Monad (contract: `0x754704Bc059F8C67012fEd69BC8A327a5aafb603`)
//! 2. Have MON for gas fees on Monad
//!
//! # Usage
//!
//! ```bash
//! PRIVATE_KEY=0xac0974... \
//! RPC_URL=https://rpc1.monad.xyz \
//! cargo run --example cross_chain_usdc -p lifiswap-evm
//! ```

use std::sync::Arc;

use lifiswap::types::{ExecutionOptions, QuoteRequest, RouteExtended};
use lifiswap::{LiFiClient, LiFiConfig};
use lifiswap_evm::{EvmProvider, LocalSigner};

const MONAD_USDC: &str = "0x754704Bc059F8C67012fEd69BC8A327a5aafb603";
const BASE_USDC: &str = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let private_key = std::env::var("PRIVATE_KEY").expect("PRIVATE_KEY env var required");
    let rpc_url: url::Url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "https://rpc1.monad.xyz".to_owned())
        .parse()?;

    let key: alloy::signers::local::PrivateKeySigner = private_key.parse()?;
    let wallet_address = format!("{:#x}", key.address());
    let signer = LocalSigner::new(key, rpc_url.clone());
    let provider = EvmProvider::new(signer, rpc_url);

    let client = LiFiClient::new(LiFiConfig::builder().integrator("lifiswap-example").build())?;
    client.add_provider(provider);

    println!("Wallet: {wallet_address}");

    let quote = client
        .get_quote(
            &QuoteRequest::builder()
                .from_chain("143") // Monad
                .from_token(MONAD_USDC)
                .from_address(&wallet_address)
                .from_amount("1000000") // 1 USDC
                .to_chain("8453") // Base
                .to_token(BASE_USDC)
                .build(),
        )
        .await?;

    println!(
        "Quote: {} → {} via {}",
        quote.action.from_token.symbol,
        quote.action.to_token.symbol,
        quote.tool.as_deref().unwrap_or("unknown"),
    );

    let update_hook = Arc::new(|route: &RouteExtended| {
        for step in &route.steps {
            if let Some(ref exec) = step.execution {
                println!("[{}] {:?}", step.id, exec.status);
                if let Some(action) = exec.actions.last() {
                    if let Some(ref msg) = action.message {
                        println!("  └─ {msg}");
                    }
                }
            }
        }
    });

    let result = client
        .execute_quote(
            quote,
            ExecutionOptions {
                update_route_hook: Some(update_hook),
                ..Default::default()
            },
        )
        .await?;

    println!("\nDone! Route {} completed.", result.id);
    for step in &result.steps {
        if let Some(ref exec) = step.execution {
            println!(
                "  Step {}: {} → {}",
                step.id,
                exec.from_amount.as_deref().unwrap_or("?"),
                exec.to_amount.as_deref().unwrap_or("?"),
            );
        }
    }

    Ok(())
}
