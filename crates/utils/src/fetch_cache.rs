use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
struct CacheEntry<T: Clone> {
    value: T,
    inserted_at: Instant,
}

/// Simple in-memory TTL cache.
pub struct FetchCache<T: Clone> {
    store: Arc<Mutex<HashMap<String, CacheEntry<T>>>>,
    ttl: Duration,
}

impl<T: Clone> FetchCache<T> {
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            store: Arc::new(Mutex::new(HashMap::new())),
            ttl: Duration::from_secs(ttl_seconds),
        }
    }

    pub fn get(&self, key: &str) -> Option<T> {
        let store = self.store.lock().ok()?;
        let entry = store.get(key)?;
        if entry.inserted_at.elapsed() > self.ttl {
            return None;
        }
        Some(entry.value.clone())
    }

    pub fn set(&self, key: String, value: T) {
        if let Ok(mut store) = self.store.lock() {
            store.insert(
                key,
                CacheEntry {
                    value,
                    inserted_at: Instant::now(),
                },
            );
        }
    }

    pub fn clear(&self) {
        if let Ok(mut store) = self.store.lock() {
            store.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_insert_and_get() {
        let cache = FetchCache::new(60);
        cache.set("key".to_string(), "value");
        assert_eq!(cache.get("key"), Some("value"));
    }

    #[test]
    fn test_cache_miss() {
        let cache: FetchCache<String> = FetchCache::new(60);
        assert_eq!(cache.get("missing"), None);
    }
}
