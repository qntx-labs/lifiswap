//! `lifiswap` CLI — cross-chain swap tool powered by `LiFi`.
#![allow(clippy::print_stdout, clippy::print_stderr)]

mod app;
mod commands;
mod output;
mod progress;

use clap::Parser;
use console::Style;

use crate::app::App;
use crate::commands::Commands;
use crate::output::OutputFormat;

#[derive(Parser)]
#[command(
    name = "lifiswap",
    version,
    about = "Cross-chain swap CLI powered by LiFi"
)]
struct Cli {
    /// Integrator name
    #[arg(long, env = "LIFI_INTEGRATOR", default_value = "lifiswap-cli")]
    integrator: String,

    /// API key for authenticated endpoints
    #[arg(long, env = "LIFI_API_KEY")]
    api_key: Option<String>,

    /// API base URL
    #[arg(long, env = "LIFI_API_URL")]
    api_url: Option<String>,

    /// Output format: table, json, compact
    #[arg(long, default_value = "table")]
    output: OutputFormat,

    /// Verbose logging (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

fn init_tracing(verbose: u8) {
    let filter = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(filter)),
        )
        .with_target(false)
        .init();
}

fn main() {
    let _ = dotenvy::dotenv();
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    if let Err(e) = rt.block_on(run(cli)) {
        let style = Style::new().red().bold();
        eprintln!("{} {e:#}", style.apply_to("error:"));
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    let app = App::new(
        &cli.integrator,
        cli.api_key.as_deref(),
        cli.api_url.as_deref(),
        cli.output,
    )?;

    cli.command.run(&app).await
}
