//! `lifiswap tokens` — search and list tokens on a chain.

use anyhow::{Result, bail};
use clap::Args;
use lifiswap::types::TokensRequest;

use crate::app::App;
use crate::output::{self, OutputFormat};
use crate::progress;

#[derive(Args)]
pub struct TokensArgs {
    /// Chain ID to list tokens for
    #[arg(long)]
    chain: u64,

    /// Filter tokens by name or symbol
    #[arg(long)]
    search: Option<String>,
}

pub async fn run(app: &App, args: TokensArgs) -> Result<()> {
    let params = TokensRequest::builder()
        .chains(args.chain.to_string())
        .build();

    let sp = progress::spinner("Fetching tokens...");
    let resp = app.client.get_tokens(Some(&params)).await?;
    sp.finish_and_clear();

    let chain_key = args.chain.to_string();
    let Some(tokens) = resp.tokens.get(&chain_key) else {
        bail!("no tokens found for chain {}", args.chain);
    };

    let filtered: Vec<_> = args.search.as_ref().map_or_else(
        || tokens.iter().collect(),
        |q| {
            let q_lower = q.to_lowercase();
            tokens
                .iter()
                .filter(|t| {
                    t.symbol.to_lowercase().contains(&q_lower)
                        || t.name.to_lowercase().contains(&q_lower)
                })
                .collect()
        },
    );

    match app.output {
        OutputFormat::Json => output::print_json(&filtered)?,
        OutputFormat::Table | OutputFormat::Compact => {
            let mut table = output::styled_table(&["Symbol", "Name", "Address", "Decimals"]);
            for t in &filtered {
                let addr = if t.address.len() > 16 {
                    format!(
                        "{}...{}",
                        &t.address[..8],
                        &t.address[t.address.len() - 6..]
                    )
                } else {
                    t.address.clone()
                };
                table.add_row(vec![
                    t.symbol.clone(),
                    t.name.clone(),
                    addr,
                    t.decimals.to_string(),
                ]);
            }
            println!("{table}");
            println!(
                "\n{}",
                output::Styles::dim().apply_to(format!("{} tokens", filtered.len()))
            );
        }
    }

    Ok(())
}
