use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Cache performance monitor
#[derive(Clone)]
pub struct CachePerformanceMonitor {
    metrics: Arc<CacheMetrics>,
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
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(CacheMetrics::default()),
        }
    }

    /// Record cache write
    pub fn record_cache_write(&self, _cagory: &str) {
        self.metrics.cache_writes.fetch_add(1, Ordering::Relaxed);
        // println!("Cache written: {}", category);
    }

    /// Record cache error
    pub fn record_cache_error(&self, category: &str, error: &str) {
        self.metrics.cache_errors.fetch_add(1, Ordering::Relaxed);
        eprintln!("Cache error in {}: {}", category, error);
    }
}

impl Default for CachePerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}
