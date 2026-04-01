use std::sync::Arc;
use std::time::Instant;

use crate::{
    cache::CacheManager,
    config::Config,
    generator::{
        context::GeneratorContext, preprocess::PreProcessAgent, types::Generator,
    },
    memory::Memory,
};
use anyhow::Result;
use tokio::sync::RwLock;

/// Memory scope and key definitions for workflow timing statistics
pub struct TimingScope;

impl TimingScope {
    /// Memory scope for timing statistics
    pub const TIMING: &'static str = "timing";
}

/// Memory key definitions for each workflow stage
pub struct TimingKeys;

impl TimingKeys {
    /// Preprocessing stage duration
    pub const PREPROCESS: &'static str = "preprocess";
    /// Total execution time
    pub const TOTAL_EXECUTION: &'static str = "total_execution";
}

pub async fn launch(c: &Config) -> Result<()> {
    let overall_start = Instant::now();

    let config = c.clone();
    let cache_manager = Arc::new(RwLock::new(CacheManager::new(
        config.cache.clone()
    )));
    let memory = Arc::new(RwLock::new(Memory::new()));

    let context = GeneratorContext {
        config,
        cache_manager,
        memory,
    };

    // Preprocessing stage
    let preprocess_start = Instant::now();
    let preprocess_agent = PreProcessAgent::new();
    preprocess_agent.execute(context.clone()).await?;
    let preprocess_time = preprocess_start.elapsed().as_secs_f64();
    context
        .store_to_memory(TimingScope::TIMING, TimingKeys::PREPROCESS, preprocess_time)
        .await?;
    println!(
        "=== Preprocessing completed, results stored to Memory (Duration: {:.2}s) ===",
        preprocess_time
    );

    // Record total execution time
    let total_time = overall_start.elapsed().as_secs_f64();
    context
        .store_to_memory(TimingScope::TIMING, TimingKeys::TOTAL_EXECUTION, total_time)
        .await?;

    println!("\n🎉 All processes execution completed! Total duration: {:.2}s", total_time);

    Ok(())
}
