use crate::generator::workflow::launch;
use anyhow::Result;
use clap::Parser;

mod cache;
mod cli;
mod config;
mod generator;
mod i18n;
mod memory;
mod types;
mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Args::parse();

    // Handle subcommands
    if let Some(command) = args.command {
        return handle_subcommand(command, args.config).await;
    }

    // Default: run documentation generation
    let config = args.to_config();
    launch(&config).await
}

/// Handle CLI subcommands
async fn handle_subcommand(command: cli::Commands, _config_path: Option<std::path::PathBuf>) -> Result<()> {
    match command {
        cli::Commands::SyncKnowledge { config: _, force: _ } => {
            println!("🛑 Knowledge base functionality is disabled.");
            Ok(())
        }
    }
}
