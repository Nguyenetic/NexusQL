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
        let kernel = NexusKernel::new(path).await?;

        let data_path = format!("{}/default", path);
        kernel
            .register_engine(Box::new(engine::default::DefaultEngine::new(&data_path)?))
            .await?;
        kernel
            .register_engine(Box::new(engine::colbert::ColBERTEngine::new()))
            .await?;
        kernel
            .register_engine(Box::new(engine::hdc::HDCEngine::new()))
            .await?;

        tracing::info!("✓ All engines registered");

        Ok(Self { kernel })
    }

    pub fn list_engines(&self) -> Vec<String> {
        self.kernel.list_engines()
    }

    pub async fn execute(&self, sql: &str) -> Result<ResultSet> {
        let statement = QueryParser::parse(sql)?;

        // Optimize query
        let optimizer = Optimizer::new();
        let optimized = optimizer.optimize(statement)?;

        // Execute optimized query
        self.kernel.execute(sql).await
    }

    pub async fn shutdown(self) -> Result<()> {
        self.kernel.shutdown().await
    }
}
