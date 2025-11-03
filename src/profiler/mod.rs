// Performance Profiler
// Built-in query performance analysis and optimization suggestions

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Query performance profile
#[derive(Debug, Clone)]
pub struct QueryProfile {
    pub query: String,
    pub execution_time: Duration,
    pub rows_scanned: usize,
    pub rows_returned: usize,
    pub index_used: Option<String>,
    pub node_breakdown: Vec<NodeProfile>,
}

#[derive(Debug, Clone)]
pub struct NodeProfile {
    pub node_type: String, // SeqScan, IndexScan, HashJoin, etc.
    pub time_ms: f64,
    pub rows_processed: usize,
}

/// Performance profiler
pub struct PerformanceProfiler {
    profiles: HashMap<String, Vec<QueryProfile>>,
    slow_query_threshold_ms: f64,
}

impl PerformanceProfiler {
    pub fn new(slow_query_threshold_ms: f64) -> Self {
        Self {
            profiles: HashMap::new(),
            slow_query_threshold_ms,
        }
    }

    /// Profile a query execution
    pub fn profile_query<F, R>(&mut self, query: &str, execute_fn: F) -> Result<R, String>
    where
        F: FnOnce() -> Result<R, String>,
    {
        let start = Instant::now();
        let result = execute_fn()?;
        let elapsed = start.elapsed();

        let profile = QueryProfile {
            query: query.to_string(),
            execution_time: elapsed,
            rows_scanned: 0, // Would be tracked in real execution
            rows_returned: 0,
            index_used: None,
            node_breakdown: Vec::new(),
        };

        // Store profile
        self.profiles
            .entry(query.to_string())
            .or_insert_with(Vec::new)
            .push(profile.clone());

        // Log slow queries
        if elapsed.as_secs_f64() * 1000.0 > self.slow_query_threshold_ms {
            self.log_slow_query(&profile);
        }

        Ok(result)
    }

    /// Analyze query performance
    pub fn analyze(&self, query: &str) -> Option<PerformanceAnalysis> {
        self.profiles.get(query).map(|profiles| {
            let execution_count = profiles.len();
            let avg_time = profiles
                .iter()
                .map(|p| p.execution_time.as_secs_f64())
                .sum::<f64>()
                / execution_count as f64;

            let min_time = profiles
                .iter()
                .map(|p| p.execution_time.as_secs_f64())
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or(0.0);

            let max_time = profiles
                .iter()
                .map(|p| p.execution_time.as_secs_f64())
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or(0.0);

            PerformanceAnalysis {
                query: query.to_string(),
                execution_count,
                avg_time_ms: avg_time * 1000.0,
                min_time_ms: min_time * 1000.0,
                max_time_ms: max_time * 1000.0,
                suggestions: self.generate_suggestions(query, avg_time * 1000.0),
            }
        })
    }

    /// Get slow queries
    pub fn get_slow_queries(&self) -> Vec<QueryProfile> {
        self.profiles
            .values()
            .flatten()
            .filter(|p| p.execution_time.as_secs_f64() * 1000.0 > self.slow_query_threshold_ms)
            .cloned()
            .collect()
    }

    /// Generate optimization suggestions
    fn generate_suggestions(&self, query: &str, avg_time_ms: f64) -> Vec<String> {
        let mut suggestions = Vec::new();
        let query_lower = query.to_lowercase();

        if avg_time_ms > 100.0 {
            suggestions.push("Query is slow (>100ms). Consider optimization.".to_string());
        }

        if query_lower.contains("select *") {
            suggestions
                .push("SELECT * scans all columns. Specify needed columns only.".to_string());
        }

        if query_lower.contains("where") && !query_lower.contains("index") {
            suggestions.push("Consider adding an index for WHERE clause columns.".to_string());
        }

        if query_lower.contains("join") && avg_time_ms > 50.0 {
            suggestions.push(
                "Large join detected. Consider using EXPLAIN to analyze join strategy.".to_string(),
            );
        }

        suggestions
    }

    fn log_slow_query(&self, profile: &QueryProfile) {
        println!(
            "[SLOW QUERY] {:.2}ms: {}",
            profile.execution_time.as_secs_f64() * 1000.0,
            profile.query
        );
    }

    pub fn stats(&self) -> ProfilerStats {
        let total_queries = self.profiles.values().map(|v| v.len()).sum();
        let slow_queries = self.get_slow_queries().len();

        ProfilerStats {
            total_queries,
            unique_queries: self.profiles.len(),
            slow_queries,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PerformanceAnalysis {
    pub query: String,
    pub execution_count: usize,
    pub avg_time_ms: f64,
    pub min_time_ms: f64,
    pub max_time_ms: f64,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ProfilerStats {
    pub total_queries: usize,
    pub unique_queries: usize,
    pub slow_queries: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profiler_basic() {
        let mut profiler = PerformanceProfiler::new(100.0);

        let result = profiler.profile_query("SELECT 1", || {
            std::thread::sleep(Duration::from_millis(10));
            Ok(42)
        });

        assert_eq!(result.unwrap(), 42);

        let stats = profiler.stats();
        assert_eq!(stats.total_queries, 1);
    }

    #[test]
    fn test_performance_analysis() {
        let mut profiler = PerformanceProfiler::new(100.0);

        // Execute same query multiple times
        for _ in 0..5 {
            profiler
                .profile_query("SELECT * FROM users", || {
                    std::thread::sleep(Duration::from_millis(20));
                    Ok(())
                })
                .unwrap();
        }

        let analysis = profiler.analyze("SELECT * FROM users").unwrap();
        assert_eq!(analysis.execution_count, 5);
        assert!(analysis.avg_time_ms > 15.0);
    }

    #[test]
    fn test_slow_query_detection() {
        let mut profiler = PerformanceProfiler::new(50.0);

        // Fast query
        profiler
            .profile_query("SELECT id FROM users", || {
                std::thread::sleep(Duration::from_millis(10));
                Ok(())
            })
            .unwrap();

        // Slow query
        profiler
            .profile_query("SELECT * FROM huge_table", || {
                std::thread::sleep(Duration::from_millis(150));
                Ok(())
            })
            .unwrap();

        let slow = profiler.get_slow_queries();
        assert_eq!(slow.len(), 1);
        assert!(slow[0].query.contains("huge_table"));
    }

    #[test]
    fn test_optimization_suggestions() {
        let mut profiler = PerformanceProfiler::new(50.0);

        profiler
            .profile_query("SELECT * FROM users WHERE name = 'test'", || {
                std::thread::sleep(Duration::from_millis(120));
                Ok(())
            })
            .unwrap();

        let analysis = profiler
            .analyze("SELECT * FROM users WHERE name = 'test'")
            .unwrap();
        assert!(analysis.suggestions.len() > 0);
    }
}
