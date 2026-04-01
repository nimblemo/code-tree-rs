use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Memory metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetadata {
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
    pub access_counts: HashMap<String, u64>,
    pub data_sizes: HashMap<String, usize>,
    pub total_size: usize,
}

impl MemoryMetadata {
    pub fn new() -> Self {
        Self {
            created_at: Utc::now(),
            last_updated: Utc::now(),
            access_counts: HashMap::new(),
            data_sizes: HashMap::new(),
            total_size: 0,
        }
    }
}

/// Unified memory manager
#[derive(Debug)]
pub struct Memory {
    data: HashMap<String, Value>,
    metadata: MemoryMetadata,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            metadata: MemoryMetadata::new(),
        }
    }

    /// Store data to specified scope and key
    pub fn store<T>(&mut self, scope: &str, key: &str, data: T) -> Result<()>
    where
        T: Serialize,
    {
        let full_key = format!("{}:{}", scope, key);
        let serialized = serde_json::to_value(data)?;

        // Calculate data size
        let data_size = serialized.to_string().len();

        // Update metadata
        if let Some(old_size) = self.metadata.data_sizes.get(&full_key) {
            self.metadata.total_size -= old_size;
        }
        self.metadata.data_sizes.insert(full_key.clone(), data_size);
        self.metadata.total_size += data_size;
        self.metadata.last_updated = Utc::now();

        self.data.insert(full_key, serialized);
        Ok(())
    }
}
