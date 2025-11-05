//! Query Result Caching - Week 5-6
//!
//! Context7 Best Practices from Redis + PostgreSQL:
//! - Cache-Aside (Lazy Loading) pattern
//! - TTL-based expiration
//! - LRU eviction policy
//! - Query fingerprinting for cache keys
//! - Configurable cache size and TTL
//! - Cache hit/miss metrics
//!
//! PROPRIETARY & PATENTABLE

use crate::types::ResultSet;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

/// Cache entry with metadata
#[derive(Debug, Clone)]
struct CacheEntry {
    /// Cached result set
    data: ResultSet,
    /// Timestamp when cached
    cached_at: u64,
    /// Time-to-live in seconds
    ttl: u64,
    /// Access count for statistics
    access_count: u64,
    /// Last access time (for LRU)
    last_access: u64,
}

impl CacheEntry {
    fn is_expired(&self) -> bool {
        let now = Self::current_timestamp();
        now - self.cached_at > self.ttl
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

/// Query cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of entries
    pub max_entries: usize,
    /// Default TTL in seconds
    pub default_ttl: u64,
    /// Enable cache (can be disabled for debugging)
    pub enabled: bool,
    /// Eviction policy
    pub eviction_policy: EvictionPolicy,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            default_ttl: 300, // 5 minutes
            enabled: true,
            eviction_policy: EvictionPolicy::LRU,
        }
    }
}

/// Eviction policy
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvictionPolicy {
    /// Least Recently Used
    LRU,
    /// Least Frequently Used
    LFU,
    /// First In First Out
    FIFO,
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Total cache hits
    pub hits: u64,
    /// Total cache misses
    pub misses: u64,
    /// Total evictions
    pub evictions: u64,
    /// Total entries cached
    pub total_cached: u64,
}

impl CacheStats {
    /// Calculate hit rate
    pub fn hit_rate(&self) -> f64 {
        if self.hits + self.misses == 0 {
            0.0
        } else {
            self.hits as f64 / (self.hits + self.misses) as f64
        }
    }
}

/// Query result cache
pub struct QueryCache {
    /// Cache storage
    cache: HashMap<u64, CacheEntry>,
    /// LRU queue (for LRU eviction)
    lru_queue: VecDeque<u64>,
    /// Configuration
    config: CacheConfig,
    /// Statistics
    stats: CacheStats,
}

impl QueryCache {
    /// Create a new cache
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: HashMap::new(),
            lru_queue: VecDeque::new(),
            config,
            stats: CacheStats::default(),
        }
    }

    /// Generate cache key from query string (hash-based)
    pub fn generate_key(query: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        query.hash(&mut hasher);
        hasher.finish()
    }

    /// Get cached result (cache-aside pattern)
    pub fn get(&mut self, query: &str) -> Option<ResultSet> {
        if !self.config.enabled {
            return None;
        }

        let key = Self::generate_key(query);

        // Check if entry exists and is not expired
        if let Some(entry) = self.cache.get_mut(&key) {
            if !entry.is_expired() {
                // Cache hit!
                self.stats.hits += 1;
                entry.access_count += 1;
                entry.last_access = CacheEntry::current_timestamp();

                // Update LRU queue
                self.lru_queue.retain(|&k| k != key);
                self.lru_queue.push_back(key);

                return Some(entry.data.clone());
            } else {
                // Expired - remove it
                self.cache.remove(&key);
                self.lru_queue.retain(|&k| k != key);
            }
        }

        // Cache miss
        self.stats.misses += 1;
        None
    }

    /// Put result in cache
    pub fn put(&mut self, query: &str, result: ResultSet) {
        self.put_with_ttl(query, result, self.config.default_ttl);
    }

    /// Put result in cache with custom TTL
    pub fn put_with_ttl(&mut self, query: &str, result: ResultSet, ttl: u64) {
        if !self.config.enabled {
            return;
        }

        let key = Self::generate_key(query);

        // Check if we need to evict
        if self.cache.len() >= self.config.max_entries && !self.cache.contains_key(&key) {
            self.evict_one();
        }

        // Create cache entry
        let entry = CacheEntry {
            data: result,
            cached_at: CacheEntry::current_timestamp(),
            ttl,
            access_count: 0,
            last_access: CacheEntry::current_timestamp(),
        };

        // Insert into cache
        self.cache.insert(key, entry);
        self.lru_queue.push_back(key);
        self.stats.total_cached += 1;
    }

    /// Evict one entry based on policy
    fn evict_one(&mut self) {
        match self.config.eviction_policy {
            EvictionPolicy::LRU => self.evict_lru(),
            EvictionPolicy::LFU => self.evict_lfu(),
            EvictionPolicy::FIFO => self.evict_fifo(),
        }
        self.stats.evictions += 1;
    }

    /// Evict least recently used entry
    fn evict_lru(&mut self) {
        if let Some(key) = self.lru_queue.pop_front() {
            self.cache.remove(&key);
        }
    }

    /// Evict least frequently used entry
    fn evict_lfu(&mut self) {
        if let Some((&key, _)) = self
            .cache
            .iter()
            .min_by_key(|(_, entry)| entry.access_count)
        {
            self.cache.remove(&key);
            self.lru_queue.retain(|&k| k != key);
        }
    }

    /// Evict first in (oldest)
    fn evict_fifo(&mut self) {
        if let Some((&key, _)) = self.cache.iter().min_by_key(|(_, entry)| entry.cached_at) {
            self.cache.remove(&key);
            self.lru_queue.retain(|&k| k != key);
        }
    }

    /// Invalidate a specific query
    pub fn invalidate(&mut self, query: &str) {
        let key = Self::generate_key(query);
        self.cache.remove(&key);
        self.lru_queue.retain(|&k| k != key);
    }

    /// Invalidate all entries for a table (pattern-based)
    pub fn invalidate_table(&mut self, _table: &str) {
        // In production, we'd need to track which queries touch which tables
        // For now, just clear all (conservative approach)
        self.clear();
    }

    /// Clear entire cache
    pub fn clear(&mut self) {
        self.cache.clear();
        self.lru_queue.clear();
    }

    /// Remove expired entries
    pub fn evict_expired(&mut self) {
        let expired_keys: Vec<u64> = self
            .cache
            .iter()
            .filter(|(_, entry)| entry.is_expired())
            .map(|(&key, _)| key)
            .collect();

        for key in expired_keys {
            self.cache.remove(&key);
            self.lru_queue.retain(|&k| k != key);
            self.stats.evictions += 1;
        }
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Get cache size
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = CacheStats::default();
    }
}

/// Query cache with automatic invalidation on table changes
pub struct SmartQueryCache {
    /// The cache
    cache: QueryCache,
    /// Table -> Query keys mapping
    table_queries: HashMap<String, Vec<u64>>,
}

impl SmartQueryCache {
    /// Create a new smart cache
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: QueryCache::new(config),
            table_queries: HashMap::new(),
        }
    }

    /// Get cached result
    pub fn get(&mut self, query: &str) -> Option<ResultSet> {
        self.cache.get(query)
    }

    /// Put result in cache with table tracking
    pub fn put(&mut self, query: &str, result: ResultSet, tables: Vec<String>) {
        let key = QueryCache::generate_key(query);

        // Track which tables this query touches
        for table in tables {
            self.table_queries
                .entry(table)
                .or_insert_with(Vec::new)
                .push(key);
        }

        self.cache.put(query, result);
    }

    /// Invalidate all queries that touch a table
    pub fn invalidate_table(&mut self, table: &str) {
        if let Some(keys) = self.table_queries.remove(table) {
            for key in keys {
                if let Some(entry) = self.cache.cache.remove(&key) {
                    self.cache.lru_queue.retain(|&k| k != key);
                    // Also remove from other table mappings
                    for queries in self.table_queries.values_mut() {
                        queries.retain(|&k| k != key);
                    }
                }
            }
        }
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> &CacheStats {
        self.cache.get_stats()
    }

    /// Clear entire cache
    pub fn clear(&mut self) {
        self.cache.clear();
        self.table_queries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::PlanCost;
    use crate::types::{ResultSet, Row, Value};

    fn create_test_result() -> ResultSet {
        ResultSet::new(vec![Row::new(
            vec!["id".to_string(), "name".to_string()],
            vec![Value::Integer(1), Value::Text("Alice".to_string())],
        )])
    }

    #[test]
    fn test_cache_hit() {
        let mut cache = QueryCache::new(CacheConfig::default());

        let query = "SELECT * FROM users WHERE id = 1";
        let result = create_test_result();

        // Cache miss initially
        assert!(cache.get(query).is_none());
        assert_eq!(cache.get_stats().misses, 1);

        // Put in cache
        cache.put(query, result.clone());

        // Cache hit now
        let cached = cache.get(query);
        assert!(cached.is_some());
        assert_eq!(cache.get_stats().hits, 1);
        assert_eq!(cached.unwrap().rows.len(), 1);
    }

    #[test]
    fn test_cache_expiration() {
        let mut cache = QueryCache::new(CacheConfig {
            default_ttl: 1, // 1 second TTL
            ..Default::default()
        });

        let query = "SELECT * FROM users";
        let result = create_test_result();

        cache.put(query, result);

        // Should be in cache
        assert!(cache.get(query).is_some());

        // Wait for expiration
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Should be expired
        assert!(cache.get(query).is_none());
    }

    #[test]
    fn test_cache_eviction_lru() {
        let mut cache = QueryCache::new(CacheConfig {
            max_entries: 2,
            eviction_policy: EvictionPolicy::LRU,
            ..Default::default()
        });

        let result = create_test_result();

        // Fill cache
        cache.put("query1", result.clone());
        cache.put("query2", result.clone());

        // Access query1 (making query2 the LRU)
        cache.get("query1");

        // Add query3 (should evict query2)
        cache.put("query3", result.clone());

        assert!(cache.get("query1").is_some());
        assert!(cache.get("query2").is_none()); // Evicted
        assert!(cache.get("query3").is_some());
        assert_eq!(cache.get_stats().evictions, 1);
    }

    #[test]
    fn test_cache_invalidation() {
        let mut cache = QueryCache::new(CacheConfig::default());

        let query = "SELECT * FROM users";
        let result = create_test_result();

        cache.put(query, result);
        assert!(cache.get(query).is_some());

        // Invalidate
        cache.invalidate(query);
        assert!(cache.get(query).is_none());
    }

    #[test]
    fn test_smart_cache_table_invalidation() {
        let mut cache = SmartQueryCache::new(CacheConfig::default());

        let result = create_test_result();

        // Cache two queries on users table
        cache.put(
            "SELECT * FROM users WHERE id = 1",
            result.clone(),
            vec!["users".to_string()],
        );
        cache.put(
            "SELECT COUNT(*) FROM users",
            result.clone(),
            vec!["users".to_string()],
        );

        // Both should be cached
        assert!(cache.get("SELECT * FROM users WHERE id = 1").is_some());
        assert!(cache.get("SELECT COUNT(*) FROM users").is_some());

        // Invalidate users table
        cache.invalidate_table("users");

        // Both should be gone
        assert!(cache.get("SELECT * FROM users WHERE id = 1").is_none());
        assert!(cache.get("SELECT COUNT(*) FROM users").is_none());
    }

    #[test]
    fn test_hit_rate_calculation() {
        let mut cache = QueryCache::new(CacheConfig::default());
        let result = create_test_result();

        cache.put("query1", result.clone());

        // 1 miss, 0 hits
        cache.get("query2");
        assert_eq!(cache.get_stats().hit_rate(), 0.0);

        // 1 miss, 1 hit
        cache.get("query1");
        assert_eq!(cache.get_stats().hit_rate(), 0.5);

        // 1 miss, 2 hits
        cache.get("query1");
        assert_eq!(cache.get_stats().hit_rate(), 2.0 / 3.0);
    }
}
