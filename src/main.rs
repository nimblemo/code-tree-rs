use crate::generator::workflow::launch;
use anyhow::Result;
use clap::Parser;

mod cache;
mod cli;
mod config;
mod generator;
mod memory;
mod types;
mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Args::parse();

    // Default: run documentation generation
    let config = args.to_config();
    launch(&config).await
}
