use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use serde_json::Value;

/// A thread-safe cache for expensive segment data (git state, kubectl context, etc.).
///
/// Keys are segment cache keys (e.g. "git_state"). Values are (last_updated, data).
/// Cache entries older than their configured TTL are considered stale.
#[derive(Clone, Default)]
pub struct SegmentCache {
    inner: Arc<RwLock<HashMap<String, CacheEntry>>>,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    updated_at: Instant,
    data: Value,
    ttl: Duration,
}

impl SegmentCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or update a cache entry.
    pub fn set(&self, key: impl Into<String>, value: Value, ttl: Duration) {
        let mut map = self.inner.write().unwrap_or_else(|e| e.into_inner());
        map.insert(
            key.into(),
            CacheEntry {
                updated_at: Instant::now(),
                data: value,
                ttl,
            },
        );
    }

    /// Get a cache entry. Returns `None` if not present or stale.
    pub fn get(&self, key: &str) -> Option<Value> {
        let map = self.inner.read().unwrap_or_else(|e| e.into_inner());
        let entry = map.get(key)?;
        if entry.updated_at.elapsed() > entry.ttl {
            return None;
        }
        Some(entry.data.clone())
    }

    /// Get regardless of staleness (for fallback display).
    pub fn get_stale(&self, key: &str) -> Option<Value> {
        let map = self.inner.read().unwrap_or_else(|e| e.into_inner());
        map.get(key).map(|e| e.data.clone())
    }

    /// Snapshot of all fresh entries as a plain HashMap for passing into RenderContext.
    pub fn snapshot(&self) -> HashMap<String, Value> {
        let map = self.inner.read().unwrap_or_else(|e| e.into_inner());
        map.iter()
            .map(|(k, e)| (k.clone(), e.data.clone()))
            .collect()
    }
}

/// Default TTL values (configurable per segment in theme TOML via cache_ttl_ms).
pub const DEFAULT_GIT_TTL_MS: u64 = 5_000;
pub const DEFAULT_KUBECTL_TTL_MS: u64 = 30_000;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn set_and_get() {
        let cache = SegmentCache::new();
        cache.set(
            "git_state",
            json!({"branch": "main"}),
            Duration::from_secs(5),
        );
        let v = cache.get("git_state").unwrap();
        assert_eq!(v["branch"], "main");
    }

    #[test]
    fn stale_entry_returns_none() {
        let cache = SegmentCache::new();
        // TTL of 0 — immediately stale.
        cache.set("k", json!("v"), Duration::from_millis(0));
        std::thread::sleep(Duration::from_millis(1));
        assert!(cache.get("k").is_none());
    }

    #[test]
    fn get_stale_returns_even_when_expired() {
        let cache = SegmentCache::new();
        cache.set("k", json!("v"), Duration::from_millis(0));
        std::thread::sleep(Duration::from_millis(1));
        assert!(cache.get_stale("k").is_some());
    }

    #[test]
    fn concurrent_reads_safe() {
        use std::thread;
        let cache = Arc::new(SegmentCache::new());
        cache.set("x", json!(1), Duration::from_secs(10));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let c = cache.clone();
                thread::spawn(move || c.get("x"))
            })
            .collect();
        for h in handles {
            assert!(h.join().unwrap().is_some());
        }
    }
}
