use anyhow::Result;
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;

use crate::config::CacheConfig;

pub mod performance_monitor;
pub use performance_monitor::CachePerformanceMonitor;

/// Cache manager
pub struct CacheManager {
    config: CacheConfig,
    performance_monitor: CachePerformanceMonitor,
}

/// Cache entry
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheEntry<T> {
    pub data: T,
    pub timestamp: u64,
    /// MD5 hash of the prompt, used for cache key generation and verification
    pub prompt_hash: String,
}

impl CacheManager {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            config,
            performance_monitor: CachePerformanceMonitor::new(),
        }
    }

    /// Generate MD5 hash of the prompt
    pub fn hash_prompt(&self, prompt: &str) -> String {
        let mut hasher = Md5::new();
        hasher.update(prompt.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Get cache file path
    fn get_cache_path(&self, category: &str, hash: &str) -> PathBuf {
        self.config
            .cache_dir
            .join(category)
            .join(format!("{}.json", hash))
    }

    pub async fn set<T>(&self, category: &str, prompt: &str, data: T) -> Result<()>
    where
        T: Serialize,
    {
        if !self.config.enabled {
            return Ok(());
        }

        let hash = self.hash_prompt(prompt);
        let cache_path = self.get_cache_path(category, &hash);

        // Ensure directory exists
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let entry = CacheEntry {
            data,
            timestamp,
            prompt_hash: hash,
        };

        match serde_json::to_string_pretty(&entry) {
            Ok(content) => match fs::write(&cache_path, content).await {
                Ok(_) => {
                    self.performance_monitor.record_cache_write(category);
                    Ok(())
                }
                Err(e) => {
                    self.performance_monitor
                        .record_cache_error(category, &format!("Failed to write file: {}", e));
                    Err(e.into())
                }
            },
            Err(e) => {
                self.performance_monitor
                    .record_cache_error(category, &format!("Serialization failed: {}", e));
                Err(e.into())
            }
        }
    }
}
