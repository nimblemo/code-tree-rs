use crate::config::{Config};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "code-tree-rs")]
#[command(
    about = "lightweight code tree generator for Rust projects"
)]
#[command(author = "Nimblemo")]
#[command(version)]
pub struct Args {
    /// Project path
    #[arg(short, long, default_value = ".")]
    pub project_path: PathBuf,

    /// Output path
    #[arg(short, long, default_value = ".tree")]
    pub output_path: PathBuf,

    /// Configuration file path
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Project name
    #[arg(short, long)]
    pub name: Option<String>,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Auto use report assistant to view report after generation
    #[arg(long, default_value = "false", action = clap::ArgAction::SetTrue)]
    pub disable_preset_tools: bool,

    /// Disable cache
    #[arg(long)]
    pub no_cache: bool,
}

impl Args {
    /// Convert CLI arguments to configuration
    pub fn to_config(self) -> Config {
        let mut config = if let Some(config_path) = &self.config {
            // If config file path is explicitly specified, load from that path
            let msg = format!("Failed to read config file from {:?}", config_path);
            Config::from_file(config_path).expect(&msg)
        } else {
            // If no config file is explicitly specified, try loading from default location
            let default_config_path = std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join(".tree.toml");

            if default_config_path.exists() {
                let msg = format!("Failed to read config file from {:?}", default_config_path);
                Config::from_file(&default_config_path).expect(&msg)
            } else {
                // Default config file doesn't exist, use default values
                Config::default()
            }
        };

        // Override settings from config file
        config.project_path = self.project_path.clone();
        config.output_path = self.output_path.clone();
        config.internal_path = self.output_path.join(".tree");

        // Project name handling: CLI argument has highest priority, if CLI doesn't specify and config file doesn't have it, get_project_name() will auto-infer
        if let Some(name) = self.name {
            config.project_name = Some(name);
        }

        // Cache configuration
        if self.no_cache {
            config.cache.enabled = true;
        }
        
        // Ensure cache directory is correctly placed under the output path
        config.cache.cache_dir = config.internal_path.clone();
        
        config.verbose = self.verbose;
        config
    }
}
