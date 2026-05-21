use std::collections::{HashMap, VecDeque};
use std::sync::LazyLock;

use tokio::sync::RwLock;

pub const DEFAULT_INDEX_CACHE_MAX_SIZE: usize = 10;

static INDEX_CACHE: LazyLock<AsyncIndexCache> =
    LazyLock::new(|| AsyncIndexCache::new(DEFAULT_INDEX_CACHE_MAX_SIZE));

#[derive(Debug)]
pub struct AsyncIndexCache {
    max_size: usize,
    inner: RwLock<CacheInner>,
}

#[derive(Debug, Default)]
struct CacheInner {
    entries: HashMap<String, String>,
    lru: VecDeque<String>,
}

impl AsyncIndexCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            inner: RwLock::new(CacheInner::default()),
        }
    }

    pub async fn get(&self, key: &str) -> Option<String> {
        let mut inner = self.inner.write().await;
        let value = inner.entries.get(key).cloned();
        if value.is_some() {
            touch_key(&mut inner.lru, key);
        }
        value
    }

    pub async fn put(&self, key: String, value: String) {
        if self.max_size == 0 {
            return;
        }

        let mut inner = self.inner.write().await;
        inner.entries.insert(key.clone(), value);
        touch_key(&mut inner.lru, &key);

        while inner.entries.len() > self.max_size {
            if let Some(evicted) = inner.lru.pop_front() {
                inner.entries.remove(&evicted);
            } else {
                break;
            }
        }
    }

    pub async fn invalidate(&self, key: &str) {
        let mut inner = self.inner.write().await;
        remove_key(&mut inner, key);
    }

    pub fn invalidate_blocking(&self, key: &str) {
        let mut inner = self.inner.blocking_write();
        remove_key(&mut inner, key);
    }
}

fn remove_key(inner: &mut CacheInner, key: &str) {
    inner.entries.remove(key);
    inner.lru.retain(|existing| existing != key);
}

fn touch_key(lru: &mut VecDeque<String>, key: &str) {
    lru.retain(|existing| existing != key);
    lru.push_back(key.to_string());
}

pub async fn get(key: &str) -> Option<String> {
    INDEX_CACHE.get(key).await
}

pub async fn put(key: String, value: String) {
    INDEX_CACHE.put(key, value).await;
}

pub async fn invalidate(key: &str) {
    INDEX_CACHE.invalidate(key).await;
}

pub fn invalidate_blocking(key: &str) {
    INDEX_CACHE.invalidate_blocking(key);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cache_evicts_least_recently_used_entry() {
        let cache = AsyncIndexCache::new(2);
        cache.put("a".to_string(), "one".to_string()).await;
        cache.put("b".to_string(), "two".to_string()).await;
        assert_eq!(cache.get("a").await, Some("one".to_string()));

        cache.put("c".to_string(), "three".to_string()).await;

        assert_eq!(cache.get("a").await, Some("one".to_string()));
        assert_eq!(cache.get("b").await, None);
        assert_eq!(cache.get("c").await, Some("three".to_string()));
    }

    #[tokio::test]
    async fn invalidate_removes_matching_entry() {
        let cache = AsyncIndexCache::new(10);
        cache
            .put("path.md".to_string(), "indexed".to_string())
            .await;

        cache.invalidate("path.md").await;

        assert_eq!(cache.get("path.md").await, None);
    }
}
