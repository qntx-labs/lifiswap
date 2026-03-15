//! `lifiswap routes` — compare multiple routes.

use anyhow::Result;
use clap::Args;
use lifiswap::types::{ChainId, RoutesRequest};

use crate::app::App;
use crate::output::{self, OutputFormat, Styles};
use crate::progress;

#[derive(Args)]
pub struct RoutesArgs {
    /// Source chain ID
    #[arg(long)]
    from_chain: u64,

    /// Source token address
    #[arg(long)]
    from_token: String,

    /// Amount in base units
    #[arg(long)]
    from_amount: String,

    /// Destination chain ID
    #[arg(long)]
    to_chain: u64,

    /// Destination token address
    #[arg(long)]
    to_token: String,

    /// Sender wallet address
    #[arg(long)]
    from_address: String,
}

pub async fn run(app: &App, args: RoutesArgs) -> Result<()> {
    let params = RoutesRequest::builder()
        .from_chain_id(ChainId(args.from_chain))
        .from_token_address(&args.from_token)
        .from_amount(&args.from_amount)
        .to_chain_id(ChainId(args.to_chain))
        .to_token_address(&args.to_token)
        .from_address(&args.from_address)
        .build();

    let sp = progress::spinner("Fetching routes...");
    let resp = app.client.get_routes(&params).await?;
    sp.finish_and_clear();

    match app.output {
        OutputFormat::Json => output::print_json(&resp)?,
        OutputFormat::Table | OutputFormat::Compact => {
            if resp.routes.is_empty() {
                println!("{}", Styles::warning().apply_to("No routes found"));
                return Ok(());
            }

            println!(
                "{}\n",
                Styles::highlight().apply_to(format!("Found {} routes", resp.routes.len()))
            );

            let mut table =
                output::styled_table(&["#", "Tools", "Output", "Gas $", "Duration", "Tags"]);

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
                let to_usd = route.to_amount_usd.as_deref().unwrap_or("?");
                let tags = route.tags.as_deref().unwrap_or(&[]).join(", ");

                table.add_row(vec![
                    (i + 1).to_string(),
                    tools.join(" + "),
                    format!("${to_usd}"),
                    format!("${gas_usd}"),
                    format!("{duration:.0}s"),
                    tags,
                ]);
            }

            println!("{table}");
        }
    }

    Ok(())
}
