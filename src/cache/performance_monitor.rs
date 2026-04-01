use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::i18n::TargetLanguage;

/// Cache performance monitor
#[derive(Clone)]
pub struct CachePerformanceMonitor {
    metrics: Arc<CacheMetrics>,
    target_language: TargetLanguage,
}

/// Cache metrics
#[derive(Default)]
pub struct CacheMetrics {
    /// Cache write count
    pub cache_writes: AtomicUsize,
    /// Cache error count
    pub cache_errors: AtomicUsize,
}

impl CachePerformanceMonitor {
    pub fn new(target_language: TargetLanguage) -> Self {
        Self {
            metrics: Arc::new(CacheMetrics::default()),
            target_language,
        }
    }

    /// Record cache write
    pub fn record_cache_write(&self, category: &str) {
        self.metrics.cache_writes.fetch_add(1, Ordering::Relaxed);
        let msg = self.target_language.msg_cache_write().replace("{}", category);
        println!("{}", msg);
    }

    /// Record cache error
    pub fn record_cache_error(&self, category: &str, error: &str) {
        self.metrics.cache_errors.fetch_add(1, Ordering::Relaxed);
        let msg = self.target_language.msg_cache_error()
            .replace("{}", category)
            .replacen("{}", error, 1);
        eprintln!("{}", msg);
    }
}

impl Default for CachePerformanceMonitor {
    fn default() -> Self {
        Self::new(TargetLanguage::default())
    }
}
