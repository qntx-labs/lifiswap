//! Query-only usage — no wallet or signer needed.
//!
//! Demonstrates read-only LI.FI API calls: chains, tokens, tools,
//! gas recommendations, and wallet balances.
//!
//! ```bash
//! cargo run --example query_only -p lifiswap-evm
//! ```

use lifiswap::types::{ChainId, ChainType, ChainsRequest, GasRecommendationRequest, TokensRequest};
use lifiswap::{LiFiClient, LiFiConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = LiFiClient::new(LiFiConfig::builder().integrator("lifiswap-example").build())?;

    // Fetch all EVM chains
    let chains = client
        .get_chains(Some(&ChainsRequest {
            chain_types: Some(vec![ChainType::EVM]),
        }))
        .await?;
    println!("EVM chains: {}", chains.len());
    for c in chains.iter().take(10) {
        println!("  {} (id={})", c.name, c.id);
    }

    // Fetch tokens on Ethereum
    let tokens = client
        .get_tokens(Some(&TokensRequest {
            chains: Some("1".to_owned()),
            chain_types: None,
            extended: None,
        }))
        .await?;
    let eth_tokens = tokens.tokens.get("1").map_or(0, |t| t.len());
    println!("\nEthereum tokens: {eth_tokens}");

    // Gas recommendation for Arbitrum
    let gas = client
        .get_gas_recommendation(&GasRecommendationRequest {
            chain_id: ChainId(42161),
            from_chain: None,
            from_token: None,
        })
        .await?;
    println!("\nArbitrum gas: {gas:?}");

    Ok(())
}
