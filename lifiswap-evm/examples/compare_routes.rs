//! Compare multiple cross-chain routes and display fee breakdowns.
//!
//! Fetches all available routes for a transfer and ranks them by output amount,
//! showing bridge tool, estimated duration, and fees for each.
//!
//! ```bash
//! cargo run --example compare_routes -p lifiswap-evm
//! ```

use lifiswap::types::{ChainId, RoutesRequest};
use lifiswap::{LiFiClient, LiFiConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = LiFiClient::new(LiFiConfig::builder().integrator("lifiswap-example").build())?;

    let resp = client
        .get_routes(
            &RoutesRequest::builder()
            .from_chain_id(ChainId(42161))  // Arbitrum
            .from_token_address("0xaf88d065e77c8cC2239327C5EDb3A432268e5831") // USDC
            .from_address("0x0000000000000000000000000000000000000001")
            .from_amount("10000000") // 10 USDC
            .to_chain_id(ChainId(8453))     // Base
            .to_token_address("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913") // USDC
            .build(),
        )
        .await?;

    println!("Found {} routes:\n", resp.routes.len());

    for (i, route) in resp.routes.iter().enumerate() {
        let tools: Vec<&str> = route
            .steps
            .iter()
            .filter_map(|s| s.tool.as_deref())
            .collect();

        let duration: f64 = route
            .steps
            .iter()
            .filter_map(|s| s.estimate.as_ref()?.execution_duration)
            .sum();

        let gas_usd = route.gas_cost_usd.as_deref().unwrap_or("?");
        let from_usd = route.from_amount_usd.as_deref().unwrap_or("?");
        let to_usd = route.to_amount_usd.as_deref().unwrap_or("?");

        println!(
            "  #{} {} → {} via [{}]",
            i + 1,
            route.from_amount,
            route.to_amount,
            tools.join(" + "),
        );
        println!("     ${from_usd} → ${to_usd}  |  gas: ${gas_usd}  |  ~{duration:.0}s");

        let tags = route.tags.as_deref().unwrap_or(&[]);
        if !tags.is_empty() {
            println!("     tags: {}", tags.join(", "));
        }
        println!();
    }

    Ok(())
}
