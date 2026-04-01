use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::{
    cache::CacheManager, 
    config::Config, 
    memory::Memory,
};

#[derive(Clone)]
pub struct GeneratorContext {
    /// Configuration
    pub config: Config,
    /// Cache manager
    pub cache_manager: Arc<RwLock<CacheManager>>,
    /// Generator memory
    pub memory: Arc<RwLock<Memory>>,
}

impl GeneratorContext {
    /// Store data to Memory
    pub async fn store_to_memory<T>(&self, scope: &str, key: &str, data: T) -> Result<()>
    where
        T: Serialize + Send + Sync,
    {
        let mut memory = self.memory.write().await;
        memory.store(scope, key, data)
    }

    /// Get data from Memory
    pub async fn get_from_memory<T>(&self, scope: &str, key: &str) -> Option<T>
    where
        T: for<'a> Deserialize<'a> + Send + Sync,
    {
        let mut memory = self.memory.write().await;
        memory.get(scope, key)
    }

    /// Check if data exists in Memory
    pub async fn has_memory_data(&self, scope: &str, key: &str) -> bool {
        let memory = self.memory.read().await;
        memory.has_data(scope, key)
    }

    /// Get all data keys within a scope
    pub async fn list_memory_keys(&self, scope: &str) -> Vec<String> {
        let memory = self.memory.read().await;
        memory.list_keys(scope)
    }

    /// Get Memory usage statistics
    pub async fn get_memory_stats(&self) -> HashMap<String, usize> {
        let memory = self.memory.read().await;
        memory.get_usage_stats()
    }
}
