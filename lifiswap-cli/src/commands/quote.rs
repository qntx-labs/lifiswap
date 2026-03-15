//! `lifiswap quote` — get a swap/bridge quote.

use anyhow::Result;
use clap::Args;
use lifiswap::types::QuoteRequest;

use crate::app::App;
use crate::output::{self, OutputFormat, Styles};
use crate::progress;

#[derive(Args)]
pub struct QuoteArgs {
    /// Source chain ID
    #[arg(long)]
    from_chain: u64,

    /// Source token address
    #[arg(long)]
    from_token: String,

    /// Amount in base units (wei/lamports/satoshis)
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

    /// Slippage tolerance (e.g. 0.03 for 3%)
    #[arg(long)]
    slippage: Option<f64>,
}

pub async fn run(app: &App, args: QuoteArgs) -> Result<()> {
    let mut params = QuoteRequest::builder()
        .from_chain(args.from_chain.to_string())
        .from_token(&args.from_token)
        .from_amount(&args.from_amount)
        .to_chain(args.to_chain.to_string())
        .to_token(&args.to_token)
        .from_address(&args.from_address)
        .build();
    params.slippage = args.slippage;

    let sp = progress::spinner("Fetching quote...");
    let step = app.client.get_quote(&params).await?;
    sp.finish_and_clear();

    match app.output {
        OutputFormat::Json => output::print_json(&step)?,
        OutputFormat::Table | OutputFormat::Compact => {
            println!("{}", Styles::highlight().apply_to("Quote"));

            if let Some(ref tool) = step.tool {
                output::print_kv("Tool", tool);
            }
            output::print_kv("Type", &format!("{:?}", step.step_type));

            let action = &step.action;
            output::print_kv(
                "From",
                &format!(
                    "{} {} (chain {})",
                    action.from_amount.as_deref().unwrap_or("?"),
                    action.from_token.symbol,
                    action.from_chain_id.0,
                ),
            );
            output::print_kv(
                "To",
                &format!(
                    "{} (chain {})",
                    action.to_token.symbol, action.to_chain_id.0,
                ),
            );

            if let Some(ref est) = step.estimate {
                if let Some(ref to_amount) = est.to_amount {
                    output::print_kv("Estimated output", to_amount);
                }
                if let Some(ref to_amount_min) = est.to_amount_min {
                    output::print_kv("Minimum output", to_amount_min);
                }
                if let Some(ref to_usd) = est.to_amount_usd {
                    output::print_kv("Output USD", &format!("${to_usd}"));
                }
                if let Some(dur) = est.execution_duration {
                    output::print_kv("Est. duration", &format!("{dur:.0}s"));
                }
                if let Some(ref gas_costs) = est.gas_costs {
                    for gc in gas_costs {
                        let usd = gc.amount_usd.as_deref().unwrap_or("?");
                        output::print_kv("Gas cost", &format!("${usd}"));
                    }
                }
                if let Some(ref fee_costs) = est.fee_costs {
                    for fc in fee_costs {
                        let usd = fc.amount_usd.as_deref().unwrap_or("?");
                        output::print_kv(&format!("Fee ({})", fc.name), &format!("${usd}"));
                    }
                }
            }
        }
    }

    Ok(())
}
