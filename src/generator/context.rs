use std::sync::Arc;

use anyhow::Result;
use serde::Serialize;
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
}
