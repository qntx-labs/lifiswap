//! `lifiswap chains` — list supported chains.

use anyhow::Result;
use clap::Args;
use lifiswap::types::{ChainType, ChainsRequest};

use crate::app::App;
use crate::output::{self, OutputFormat};
use crate::progress;

#[derive(Args)]
pub struct ChainsArgs {
    /// Filter by chain type (evm, svm, utxo)
    #[arg(long, value_parser = parse_chain_type)]
    r#type: Option<ChainType>,
}

fn parse_chain_type(s: &str) -> Result<ChainType, String> {
    match s.to_lowercase().as_str() {
        "evm" => Ok(ChainType::EVM),
        "svm" | "solana" => Ok(ChainType::SVM),
        "utxo" | "btc" | "bitcoin" => Ok(ChainType::UTXO),
        "mvm" | "sui" => Ok(ChainType::MVM),
        other => Err(format!("unknown chain type: {other}")),
    }
}

pub async fn run(app: &App, args: ChainsArgs) -> Result<()> {
    let params = args.r#type.map(|t| ChainsRequest {
        chain_types: Some(vec![t]),
    });

    let sp = progress::spinner("Fetching chains...");
    let chains = app.client.get_chains(params.as_ref()).await?;
    sp.finish_and_clear();

    match app.output {
        OutputFormat::Json => output::print_json(&chains)?,
        OutputFormat::Table | OutputFormat::Compact => {
            let mut table = output::styled_table(&["ID", "Name", "Type", "Coin", "Mainnet"]);
            for c in &chains {
                table.add_row(vec![
                    c.id.0.to_string(),
                    c.name.clone(),
                    format!("{:?}", c.chain_type),
                    c.coin.clone().unwrap_or_default(),
                    if c.mainnet { "✓" } else { "" }.to_owned(),
                ]);
            }
            println!("{table}");
            println!(
                "\n{}",
                output::Styles::dim().apply_to(format!("{} chains", chains.len()))
            );
        }
    }

    Ok(())
}
