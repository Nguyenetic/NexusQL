//! Storage Engine Interface
//! PROPRIETARY & PATENTABLE

pub mod aggregation;
pub mod capabilities;
pub mod colbert;
pub mod default;
pub mod executor;
pub mod hdc;
pub mod sort;

use crate::{
    types::{ResultSet, Value},
    Result,
};
pub use aggregation::{AggregateFunction, AggregationExecutor};
use async_trait::async_trait;
pub use capabilities::*;
pub use executor::Executor;
pub use sort::{SortDirection, SortExecutor, SortKey};

#[derive(Debug, Clone)]
pub struct Query {
    pub sql: String,
    pub params: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct QueryCost {
    pub latency_ms: f64,
    pub memory_mb: f64,
    pub energy_mw: f64,
    pub accuracy: f64,
}

impl QueryCost {
    pub fn total_score(&self) -> f64 {
        self.latency_ms * 0.4 + self.memory_mb * 0.3 + self.energy_mw * 0.3
    }
}

#[async_trait]
pub trait StorageEngine: Send + Sync {
    fn name(&self) -> &str;
    fn capabilities(&self) -> EngineCapabilities;
    async fn execute(&self, query: &Query) -> Result<ResultSet>;
    fn estimate_cost(&self, query: &Query) -> QueryCost;
    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }
    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }

    /// Enable/disable bulk-load mode: when ON, DDL/DML statements skip the
    /// per-statement disk persistence (huge O(N^2) savings on wide bulk
    /// INSERTs). Tables touched while bulk mode is on must be made durable
    /// by a subsequent `flush()` call. Default impl: no-op.
    fn set_bulk_mode(&self, _enabled: bool) {}

    /// Force any deferred writes to disk. No-op for engines that don't buffer.
    async fn flush(&self) -> Result<()> {
        Ok(())
    }
}
pub mod batch;
pub use batch::BatchInsert;
pub mod bulk_loader;
pub use bulk_loader::BulkLoader;
pub mod fast_batch;
pub use fast_batch::{BatchPool, FastBatchInsert};
pub mod optimized_batch;
pub use optimized_batch::{
    EngineStats as BatchEngineStats, OptimizedBatchConfig, OptimizedBatchEngine,
};

// Week 3-4: Advanced Query Optimization & Distributed Execution
pub mod advanced_optimizer;
pub use advanced_optimizer::{
    AdvancedOptimizer, ColumnStats, JoinAlgorithm, PlanCost, PlanNode, ScanMethod, TableStats,
};
pub mod join_optimizer;
pub use join_optimizer::{JoinCondition, JoinGraph, JoinOrderOptimizer, Relation};
pub mod distributed;
pub use distributed::{
    DistributedPlan, DistributedPlanner, DistributedTable, DistributionStrategy, PlannerType,
    ShardInfo, WorkerNode,
};
