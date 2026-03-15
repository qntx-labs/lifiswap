//! `lifiswap status` — check transaction execution status.

use anyhow::Result;
use clap::Args;
use lifiswap::types::{ChainId, StatusRequest};

use crate::app::App;
use crate::output::{self, OutputFormat, Styles};
use crate::progress;

#[derive(Args)]
pub struct StatusArgs {
    /// Transaction hash
    #[arg(long)]
    tx_hash: String,

    /// Bridge tool used
    #[arg(long)]
    bridge: Option<String>,

    /// Source chain ID
    #[arg(long)]
    from_chain: Option<u64>,

    /// Destination chain ID
    #[arg(long)]
    to_chain: Option<u64>,
}

pub async fn run(app: &App, args: StatusArgs) -> Result<()> {
    let params = StatusRequest {
        tx_hash: Some(args.tx_hash.clone()),
        task_id: None,
        bridge: args.bridge.clone(),
        from_chain: args.from_chain.map(ChainId),
        to_chain: args.to_chain.map(ChainId),
    };

    let sp = progress::spinner("Checking status...");
    let resp = app.client.get_status(&params).await?;
    sp.finish_and_clear();

    match app.output {
        OutputFormat::Json => output::print_json(&resp)?,
        OutputFormat::Table | OutputFormat::Compact => {
            let status_style = match resp.status {
                lifiswap::types::TransferStatus::Done => Styles::success(),
                lifiswap::types::TransferStatus::Failed
                | lifiswap::types::TransferStatus::Invalid => Styles::error(),
                _ => Styles::warning(),
            };
            output::print_kv(
                "Status",
                &status_style
                    .apply_to(format!("{:?}", resp.status))
                    .to_string(),
            );

            if let Some(ref sub) = resp.substatus {
                output::print_kv("Substatus", sub);
            }
            if let Some(ref msg) = resp.substatus_message {
                output::print_kv("Message", msg);
            }
            if let Some(ref tool) = resp.tool {
                output::print_kv("Tool", tool);
            }

            if let Some(ref sending) = resp.sending {
                println!("\n{}", Styles::highlight().apply_to("Sending"));
                if let Some(ref hash) = sending.tx_hash {
                    output::print_kv("  Tx Hash", hash);
                }
                if let Some(ref link) = sending.tx_link {
                    output::print_kv("  Explorer", link);
                }
                if let Some(ref token) = sending.token {
                    let amount = sending.amount.as_deref().unwrap_or("?");
                    output::print_kv("  Amount", &format!("{amount} {}", token.symbol));
                }
            }

            if let Some(ref receiving) = resp.receiving {
                println!("\n{}", Styles::highlight().apply_to("Receiving"));
                if let Some(ref hash) = receiving.tx_hash {
                    output::print_kv("  Tx Hash", hash);
                }
                if let Some(ref link) = receiving.tx_link {
                    output::print_kv("  Explorer", link);
                }
                if let Some(ref token) = receiving.token {
                    let amount = receiving.amount.as_deref().unwrap_or("?");
                    output::print_kv("  Amount", &format!("{amount} {}", token.symbol));
                }
            }

            if let Some(ref link) = resp.lifi_explorer_link {
                println!();
                output::print_kv("LiFi Explorer", link);
            }
        }
    }

    Ok(())
}
