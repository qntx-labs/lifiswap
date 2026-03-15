//! `lifiswap connections` — show available connections between chains.

use anyhow::Result;
use clap::Args;
use lifiswap::types::{ChainId, ConnectionsRequest};

use crate::app::App;
use crate::output::{self, OutputFormat};
use crate::progress;

#[derive(Args)]
pub struct ConnectionsArgs {
    /// Source chain ID
    #[arg(long)]
    from_chain: u64,

    /// Destination chain ID
    #[arg(long)]
    to_chain: u64,

    /// Filter by source token address
    #[arg(long)]
    from_token: Option<String>,

    /// Filter by destination token address
    #[arg(long)]
    to_token: Option<String>,
}

pub async fn run(app: &App, args: ConnectionsArgs) -> Result<()> {
    let params = ConnectionsRequest {
        from_chain: Some(ChainId(args.from_chain)),
        to_chain: Some(ChainId(args.to_chain)),
        from_token: args.from_token,
        to_token: args.to_token,
        ..ConnectionsRequest::default()
    };

    let sp = progress::spinner("Fetching connections...");
    let resp = app.client.get_connections(&params).await?;
    sp.finish_and_clear();

    match app.output {
        OutputFormat::Json => output::print_json(&resp)?,
        OutputFormat::Table | OutputFormat::Compact => {
            let mut table =
                output::styled_table(&["From Chain", "To Chain", "From Tokens", "To Tokens"]);
            for c in &resp.connections {
                table.add_row(vec![
                    c.from_chain_id.0.to_string(),
                    c.to_chain_id.0.to_string(),
                    c.from_tokens.len().to_string(),
                    c.to_tokens.len().to_string(),
                ]);
            }
            println!("{table}");
            println!(
                "\n{}",
                output::Styles::dim().apply_to(format!("{} connections", resp.connections.len()))
            );
        }
    }

    Ok(())
}
