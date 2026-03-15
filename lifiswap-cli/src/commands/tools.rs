//! `lifiswap tools` — list available bridges and exchanges.

use anyhow::Result;
use clap::Args;

use crate::app::App;
use crate::output::{self, OutputFormat};
use crate::progress;

#[derive(Args)]
pub struct ToolsArgs;

pub async fn run(app: &App, _args: ToolsArgs) -> Result<()> {
    let sp = progress::spinner("Fetching tools...");
    let resp = app.client.get_tools(None).await?;
    sp.finish_and_clear();

    match app.output {
        OutputFormat::Json => output::print_json(&resp)?,
        OutputFormat::Table | OutputFormat::Compact => {
            println!("{}", output::Styles::highlight().apply_to("Bridges"));
            let mut table = output::styled_table(&["Key", "Name", "Chains"]);
            for b in &resp.bridges {
                table.add_row(vec![
                    b.key.clone(),
                    b.name.clone(),
                    b.supported_chains
                        .as_ref()
                        .map_or_else(|| String::from("—"), |c| c.len().to_string()),
                ]);
            }
            println!("{table}");
            println!(
                "{}",
                output::Styles::dim().apply_to(format!("{} bridges", resp.bridges.len()))
            );

            println!("\n{}", output::Styles::highlight().apply_to("Exchanges"));
            let mut table = output::styled_table(&["Key", "Name", "Chains"]);
            for e in &resp.exchanges {
                table.add_row(vec![
                    e.key.clone(),
                    e.name.clone(),
                    e.supported_chains
                        .as_ref()
                        .map_or_else(|| String::from("—"), |c| c.len().to_string()),
                ]);
            }
            println!("{table}");
            println!(
                "{}",
                output::Styles::dim().apply_to(format!("{} exchanges", resp.exchanges.len()))
            );
        }
    }

    Ok(())
}
