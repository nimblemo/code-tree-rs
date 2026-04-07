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
    let mut args = cli::Args::parse();
    // Take command out before args is consumed by to_config()
    let command = args.command.take();
    let config = args.to_config();

    match command {
        Some(cli::Commands::Stats { path, json, tree, dump, depth, filter }) => {
            if tree {
                generator::preprocess::extractors::dir_tree_extractor::run_tree(&config, &path, json, dump, depth, filter).await
            } else {
                generator::preprocess::extractors::dir_stats_extractor::run_stats(&config, &path, json, dump).await
            }
        }
        None => launch(&config).await,
    }
}
