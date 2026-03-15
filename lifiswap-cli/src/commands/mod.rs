//! CLI subcommand definitions and dispatch.

pub mod chains;
pub mod connections;
pub mod gas;
pub mod quote;
pub mod routes;
pub mod status;
pub mod tokens;
pub mod tools;

use clap::Subcommand;

use crate::app::App;

#[derive(Subcommand)]
pub enum Commands {
    /// List supported chains
    Chains(chains::ChainsArgs),
    /// Search and list tokens on a chain
    Tokens(tokens::TokensArgs),
    /// List available bridges and exchanges
    Tools(tools::ToolsArgs),
    /// Show available connections between chains
    Connections(connections::ConnectionsArgs),
    /// Get gas recommendation for a chain
    Gas(gas::GasArgs),
    /// Check transaction execution status
    Status(status::StatusArgs),
    /// Get a swap/bridge quote
    Quote(quote::QuoteArgs),
    /// Compare multiple routes
    Routes(routes::RoutesArgs),
}

impl Commands {
    pub async fn run(self, app: &App) -> anyhow::Result<()> {
        match self {
            Self::Chains(args) => chains::run(app, args).await,
            Self::Tokens(args) => tokens::run(app, args).await,
            Self::Tools(args) => tools::run(app, args).await,
            Self::Connections(args) => connections::run(app, args).await,
            Self::Gas(args) => gas::run(app, args).await,
            Self::Status(args) => status::run(app, args).await,
            Self::Quote(args) => quote::run(app, args).await,
            Self::Routes(args) => routes::run(app, args).await,
        }
    }
}
