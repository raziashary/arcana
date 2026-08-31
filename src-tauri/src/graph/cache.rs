// Cache layer — query result caching with LRU eviction

use crate::errors::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct CacheEntry<T> {
    pub data: T,
    pub created_at: Instant,
    pub accessed_at: Instant,
    pub hit_count: u32,
}

pub struct QueryCache<T> {
    cache: Arc<Mutex<HashMap<String, CacheEntry<T>>>>,
    max_entries: usize,
    ttl: Duration,
}

impl<T: Clone> QueryCache<T> {
    pub fn new(max_entries: usize, ttl_secs: u64) -> Self {
        QueryCache {
            cache: Arc::new(Mutex::new(HashMap::new())),
            max_entries,
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    pub fn get(&self, key: &str) -> Option<T> {
        let mut cache = self.cache.lock().unwrap();

        if let Some(entry) = cache.get_mut(key) {
            // Check if expired
            if entry.created_at.elapsed() > self.ttl {
                cache.remove(key);
                return None;
            }

            // Update access time and hit count
            entry.accessed_at = Instant::now();
            entry.hit_count += 1;
            return Some(entry.data.clone());
        }

        None
    }

    pub fn set(&self, key: String, data: T) {
        let mut cache = self.cache.lock().unwrap();

        // Evict LRU entry if at capacity
        if cache.len() >= self.max_entries && !cache.contains_key(&key) {
            if let Some(lru_key) = cache
                .iter()
                .min_by_key(|(_, entry)| entry.accessed_at)
                .map(|(k, _)| k.clone())
            {
                cache.remove(&lru_key);
            }
        }

        let entry = CacheEntry {
            data,
            created_at: Instant::now(),
            accessed_at: Instant::now(),
            hit_count: 0,
        };

        cache.insert(key, entry);
    }

    pub fn invalidate(&self, key: &str) {
        let mut cache = self.cache.lock().unwrap();
        cache.remove(key);
    }

    pub fn clear(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
    }

    pub fn size(&self) -> usize {
        let cache = self.cache.lock().unwrap();
        cache.len()
    }

    pub fn stats(&self) -> CacheStats {
        let cache = self.cache.lock().unwrap();
        let total_hits: u32 = cache.values().map(|e| e.hit_count).sum();
        let avg_hits = if cache.is_empty() {
            0.0
        } else {
            total_hits as f32 / cache.len() as f32
        };

        CacheStats {
            entries: cache.len(),
            total_hits,
            avg_hits,
            max_capacity: self.max_entries,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub entries: usize,
    pub total_hits: u32,
    pub avg_hits: f32,
    pub max_capacity: usize,
}
