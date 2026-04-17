//! NexusSQL - Brain-Inspired Multi-Modal Intelligence Database
//! PROPRIETARY & CONFIDENTIAL

pub mod engine;
pub mod error;
pub mod index;
pub mod kernel;
pub mod optimizer;
pub mod query;
pub mod router;
pub mod storage;
pub mod types;
pub mod vector;

// Week 5-6: Advanced Features
pub mod adaptive;
pub mod cache;
pub mod materialized_view;
pub mod parallel;

// Week 7-9: Database Fundamentals
pub mod auth;
pub mod schema;
pub mod transaction;

// Week 10: Convenience Features
pub mod procedure;
pub mod sequence;
pub mod trigger;
pub mod view;

// Week 13: Aggregate & Window Functions
pub mod aggregate;
pub mod window;

// Week 12: Table Partitioning
pub mod partition;

// Week 14: Joins & Subqueries
pub mod join;

// Week 15: CTEs & Recursive Queries
pub mod cte;

// Week 16: JSON/JSONB & Arrays
pub mod jsonb;

// Week 17: Full-Text Search
pub mod fulltext;

// Week 18: Query Planner & Statistics
pub mod planner;

// Week 19: VACUUM & Storage Management
pub mod vacuum;

// Week 20-21: Replication & Enterprise Features
pub mod replication;

// v1.1.0: Extensions & Plugins
pub mod extensions;

// v1.2.0: Advanced Analytics
pub mod analytics;

// v1.3.0: Distributed Database
pub mod distributed;

// v2.0.0: AI-Native Database
pub mod ai_native;

// Advanced SQL Features
pub mod advanced;

// CLI Tool
pub mod cli;

// Performance Profiler
pub mod profiler;

// Migrations
pub mod migrations;

// Backup & Restore
pub mod backup;

// Security & Audit
pub mod security;

// Developer Tools
pub mod devtools;

pub use engine::StorageEngine;
pub use error::{NexusError, Result};
pub use index::HNSWIndex;
pub use kernel::NexusKernel;
pub use optimizer::Optimizer;
pub use query::{QueryParser, Statement};
pub use storage::LSMStorage;
pub use types::{ResultSet, Row, Value};

/// Main NexusSQL database instance
///
/// NexusQL is a production-ready, PostgreSQL-compatible, AI-native database
/// with advanced features including:
/// - Full ACID transactions with MVCC
/// - 5 index types (B-Tree, HNSW, GIN, GiST, BRIN)
/// - Table partitioning and sharding
/// - Full-text search and semantic search
/// - Replication and high availability
///
/// # Examples
///
/// ```no_run
/// use nexusql::NexusSQL;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let db = NexusSQL::new("./mydb").await?;
///     db.execute("CREATE TABLE users (id INT, name TEXT)").await?;
///     let result = db.execute("SELECT * FROM users").await?;
///     db.shutdown().await?;
///     Ok(())
/// }
/// ```
pub struct NexusSQL {
    kernel: NexusKernel,
}

impl NexusSQL {
    pub async fn new(path: &str) -> Result<Self> {
        // Check if in-memory database requested
        let is_memory = path == ":memory:";

        // For in-memory, use a unique temp path (won't actually create files)
        let actual_path = if is_memory {
            // Use a unique ID to avoid collisions between multiple :memory: instances
            use std::sync::atomic::{AtomicU64, Ordering};
            static MEMORY_COUNTER: AtomicU64 = AtomicU64::new(0);
            let id = MEMORY_COUNTER.fetch_add(1, Ordering::SeqCst);
            format!(":memory:_{}", id)
        } else {
            path.to_string()
        };

        let kernel = NexusKernel::new(&actual_path, is_memory).await?;

        let data_path = if is_memory {
            format!("{}:default", actual_path) // Special marker for in-memory
        } else {
            format!("{}/default", actual_path)
        };

        kernel
            .register_engine(Box::new(engine::default::DefaultEngine::new(
                &data_path, is_memory,
            )?))
            .await?;
        kernel
            .register_engine(Box::new(engine::colbert::ColBERTEngine::new()))
            .await?;
        kernel
            .register_engine(Box::new(engine::hdc::HDCEngine::new()))
            .await?;

        tracing::info!(
            "✓ All engines registered (mode: {})",
            if is_memory { "memory" } else { "disk" }
        );

        Ok(Self { kernel })
    }

    pub fn list_engines(&self) -> Vec<String> {
        self.kernel.list_engines()
    }

    /// Execute SQL with optional parameters
    ///
    /// # Arguments
    /// * `sql` - SQL statement with optional parameter placeholders ($1, $2, etc.)
    /// * `params` - Optional vector of parameter values (positional, 1-indexed)
    ///
    /// # Examples
    /// ```no_run
    /// # use nexusql::{NexusSQL, Value};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let db = NexusSQL::new("./mydb").await?;
    ///
    /// // Without parameters
    /// db.execute_with_params("CREATE TABLE users (id INT, name TEXT)", None).await?;
    ///
    /// // With positional parameters
    /// db.execute_with_params(
    ///     "INSERT INTO users VALUES ($1, $2)",
    ///     Some(vec![Value::Integer(1), Value::Text("Alice".to_string())])
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute_with_params(
        &self,
        sql: &str,
        params: Option<Vec<Value>>,
    ) -> Result<ResultSet> {
        // Previously this path parsed the SQL twice and ran the optimizer
        // whose result was immediately discarded (`let _optimized`). That
        // doubled parse cost on every call — measurable (~50%) even for
        // small queries, dominant for wide bulk INSERTs with hundreds of
        // VALUES tuples. The kernel re-parses on its own path, so the work
        // here is strictly redundant; drop it.
        //
        // If/when the optimizer gains real effect (pushdown, CSE, join
        // reordering), reintroduce it via `self.kernel.execute_plan(plan)`
        // so we don't pay parse twice.
        self.kernel
            .execute_with_params(sql, params.unwrap_or_default())
            .await
    }

    /// Execute SQL without parameters (backward compatible)
    pub async fn execute(&self, sql: &str) -> Result<ResultSet> {
        self.execute_with_params(sql, None).await
    }

    /// Toggle bulk-load mode across every registered engine. When ON, DDL/DML
    /// skips per-statement disk persistence — table state lives in memory
    /// until `flush()` is called. Single biggest speedup for large loads.
    pub async fn set_bulk_mode(&self, enabled: bool) {
        self.kernel.set_bulk_mode(enabled).await
    }

    /// Flush any deferred writes across all engines. Call after bulk-loading.
    pub async fn flush(&self) -> Result<()> {
        self.kernel.flush().await
    }

    pub async fn shutdown(self) -> Result<()> {
        self.kernel.shutdown().await
    }
}

// ============================================================================
// Backwards Compatibility Layer
// ============================================================================

/// Type alias for backwards compatibility with v2.x API
///
/// # Deprecation Notice
///
/// `Database` has been renamed to `NexusSQL` in v3.x for better clarity.
/// This alias is provided for backwards compatibility but will be removed in v4.0.
///
/// # Migration
///
/// Please update your code:
///
/// ```rust,ignore
/// // Old (v2.x) - DEPRECATED
/// use nexusql::Database;
/// let db = Database::open("data.db").await?;
///
/// // New (v3.x) - RECOMMENDED
/// use nexusql::NexusSQL;
/// let db = NexusSQL::new("data.db").await?;
/// ```
///
/// See [MIGRATION.md](../MIGRATION.md) for complete migration guide.
#[deprecated(
    since = "3.0.0",
    note = "Use `NexusSQL` instead. See MIGRATION.md for migration guide."
)]
pub type Database = NexusSQL;

#[allow(deprecated)]
impl Database {
    /// Open a database (deprecated method name)
    ///
    /// # Deprecation Notice
    ///
    /// Use `NexusSQL::new()` instead. This method is provided for backwards
    /// compatibility only and will be removed in v4.0.
    ///
    /// # Migration
    ///
    /// ```rust,ignore
    /// // Old
    /// let db = Database::open("data.db").await?;
    ///
    /// // New
    /// let db = NexusSQL::new("data.db").await?;
    /// ```
    #[deprecated(
        since = "3.0.0",
        note = "Use `NexusSQL::new()` instead. See MIGRATION.md for migration guide."
    )]
    pub async fn open(path: &str) -> Result<Self> {
        NexusSQL::new(path).await
    }
}
