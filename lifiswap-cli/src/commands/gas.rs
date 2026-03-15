//! `lifiswap gas` — get gas recommendation for a chain.

use anyhow::Result;
use clap::Args;
use lifiswap::types::{ChainId, GasRecommendationRequest};

use crate::app::App;
use crate::output::{self, OutputFormat};
use crate::progress;

#[derive(Args)]
pub struct GasArgs {
    /// Chain ID
    #[arg(long)]
    chain: u64,
}

pub async fn run(app: &App, args: GasArgs) -> Result<()> {
    let params = GasRecommendationRequest::builder()
        .chain_id(ChainId(args.chain))
        .build();

    let sp = progress::spinner("Fetching gas recommendation...");
    let resp = app.client.get_gas_recommendation(&params).await?;
    sp.finish_and_clear();

    match app.output {
        OutputFormat::Json => output::print_json(&resp)?,
        OutputFormat::Table | OutputFormat::Compact => {
            output::print_kv("Chain", &args.chain.to_string());
            if let Some(ref recommended) = resp.recommended {
                output::print_kv(
                    "Recommended",
                    &format!(
                        "{} (${} USD)",
                        recommended.amount.as_deref().unwrap_or("?"),
                        recommended.amount_usd.as_deref().unwrap_or("?"),
                    ),
                );
            }
            if let Some(ref slow) = resp.slow {
                output::print_kv("Slow", slow.amount.as_deref().unwrap_or("?"));
            }
            if let Some(ref avg) = resp.average {
                output::print_kv("Average", avg.amount.as_deref().unwrap_or("?"));
            }
            if let Some(ref fast) = resp.fast {
                output::print_kv("Fast", fast.amount.as_deref().unwrap_or("?"));
            }
        }
    }

    Ok(())
}
