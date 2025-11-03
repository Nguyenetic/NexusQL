//! Distributed Query Execution - Week 3-4
//!
//! Context7 Best Practices from Citus (PostgreSQL extension):
//! - Shard-based data distribution (hash or range)
//! - Parallel query execution across workers
//! - Smart query routing (router planner for single-shard queries)
//! - Adaptive executor with slow-start connection management
//! - Co-located joins for performance
//!
//! PROPRIETARY & PATENTABLE

use crate::engine::advanced_optimizer::{PlanCost, PlanNode};
use crate::types::Value;
use std::collections::HashMap;

/// Distribution strategy (Citus patterns)
#[derive(Debug, Clone, PartialEq)]
pub enum DistributionStrategy {
    /// Hash distribution (most common in Citus)
    /// Data is sharded by hash(column) % shard_count
    Hash { column: String },
    /// Range distribution (time-series data)
    /// Data is sharded by column value ranges
    Range {
        column: String,
        ranges: Vec<(Value, Value)>,
    },
    /// Replicated (reference tables in Citus)
    /// Full copy on every worker node
    Replicated,
    /// Single-node (local tables)
    Local,
}

/// Shard metadata
#[derive(Debug, Clone)]
pub struct ShardInfo {
    /// Shard ID
    pub id: u64,
    /// Which worker node holds this shard
    pub worker_id: String,
    /// Shard range (for range distribution)
    pub range: Option<(Value, Value)>,
    /// Estimated row count
    pub row_count: u64,
}

/// Table distribution metadata
#[derive(Debug, Clone)]
pub struct DistributedTable {
    /// Table name
    pub name: String,
    /// Distribution strategy
    pub strategy: DistributionStrategy,
    /// All shards for this table
    pub shards: Vec<ShardInfo>,
}

impl DistributedTable {
    /// Get shard for a specific distribution column value
    pub fn get_shard_for_value(&self, value: &Value) -> Option<&ShardInfo> {
        match &self.strategy {
            DistributionStrategy::Hash { .. } => {
                // Hash the value and mod by shard count
                let hash = self.hash_value(value);
                let shard_idx = (hash % self.shards.len() as u64) as usize;
                Some(&self.shards[shard_idx])
            }
            DistributionStrategy::Range { .. } => {
                // Find shard whose range contains the value
                // TODO: Implement proper Value comparison when needed
                self.shards.iter().find(|s| s.range.is_some())
            }
            DistributionStrategy::Replicated => {
                // Can use any shard (pick first)
                self.shards.first()
            }
            DistributionStrategy::Local => {
                // Single shard
                self.shards.first()
            }
        }
    }

    /// Simple hash function for distribution
    fn hash_value(&self, value: &Value) -> u64 {
        // TODO: Use a proper hash function like FNV or xxHash
        // For now, simple string-based hash
        let s = format!("{:?}", value);
        let mut hash = 0u64;
        for byte in s.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
        }
        hash
    }

    /// Check if two tables are co-located (same distribution column and shard count)
    pub fn is_colocated_with(&self, other: &DistributedTable) -> bool {
        match (&self.strategy, &other.strategy) {
            (
                DistributionStrategy::Hash { column: c1 },
                DistributionStrategy::Hash { column: c2 },
            ) => c1 == c2 && self.shards.len() == other.shards.len(),
            _ => false,
        }
    }
}

/// Worker node information
#[derive(Debug, Clone)]
pub struct WorkerNode {
    pub id: String,
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
}

/// Distributed query plan
#[derive(Debug, Clone)]
pub enum DistributedPlan {
    /// Router query: entire query runs on single worker
    Router {
        worker_id: String,
        local_plan: Box<PlanNode>,
    },
    /// Parallel query: runs across multiple workers
    Parallel {
        worker_plans: HashMap<String, PlanNode>,
        coordinator_plan: Option<Box<PlanNode>>,
    },
    /// Broadcast-join: broadcast small table to all workers
    BroadcastJoin {
        broadcast_table: String,
        worker_plans: HashMap<String, PlanNode>,
    },
}

/// Query planner type (Citus has 4 planners)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannerType {
    /// Fast path: simple CRUD on single distribution value
    FastPath,
    /// Router: complex query on single co-located shard set
    Router,
    /// LogicalPushdown: push entire query to workers
    LogicalPushdown,
    /// LogicalJoinOrder: optimize non-co-located joins
    LogicalJoinOrder,
}

/// Distributed query planner
pub struct DistributedPlanner {
    /// Table distribution metadata
    tables: HashMap<String, DistributedTable>,
    /// Worker nodes
    workers: Vec<WorkerNode>,
    /// Adaptive executor config
    slow_start_initial_conns: usize,
    slow_start_interval_ms: u64,
}

impl DistributedPlanner {
    pub fn new() -> Self {
        Self {
            tables: HashMap::new(),
            workers: Vec::new(),
            slow_start_initial_conns: 1, // Citus default
            slow_start_interval_ms: 10,  // Citus: every 10ms
        }
    }

    /// Register a distributed table
    pub fn register_table(&mut self, table: DistributedTable) {
        self.tables.insert(table.name.clone(), table);
    }

    /// Add a worker node
    pub fn add_worker(&mut self, worker: WorkerNode) {
        self.workers.push(worker);
    }

    /// Plan a distributed query (Citus approach)
    pub fn plan_query(
        &self,
        plan: &PlanNode,
        filter_values: &HashMap<String, Value>,
    ) -> DistributedPlan {
        // 1. Try fast path planner
        if let Some(router_plan) = self.try_fast_path(plan, filter_values) {
            return router_plan;
        }

        // 2. Try router planner
        if let Some(router_plan) = self.try_router_planner(plan, filter_values) {
            return router_plan;
        }

        // 3. Try logical pushdown (full query to each worker)
        if self.can_pushdown_fully(plan) {
            return self.plan_parallel_pushdown(plan);
        }

        // 4. Use logical join order planner (repartition joins)
        self.plan_repartition_join(plan)
    }

    /// Fast path planner: simple point query
    fn try_fast_path(
        &self,
        plan: &PlanNode,
        filter_values: &HashMap<String, Value>,
    ) -> Option<DistributedPlan> {
        // Check if this is a simple scan with equality filter on distribution column
        match plan {
            PlanNode::Scan { table,  .. } => {
                let dist_table = self.tables.get(table)?;

                // Get distribution column
                let dist_col = match &dist_table.strategy {
                    DistributionStrategy::Hash { column } => column,
                    _ => return None,
                };

                // Check if filter contains distribution column value
                let value = filter_values.get(dist_col)?;

                // Get target shard
                let shard = dist_table.get_shard_for_value(value)?;

                Some(DistributedPlan::Router {
                    worker_id: shard.worker_id.clone(),
                    local_plan: Box::new(plan.clone()),
                })
            }
            _ => None,
        }
    }

    /// Router planner: query can run on single worker (co-located data)
    fn try_router_planner(
        &self,
        plan: &PlanNode,
        filter_values: &HashMap<String, Value>,
    ) -> Option<DistributedPlan> {
        // Check if all tables in query are co-located and filtered by same distribution value
        let tables = self.extract_tables(plan);

        if tables.is_empty() {
            return None;
        }

        // Get first table's distribution
        let first_table = self.tables.get(&tables[0])?;

        // Check all tables are co-located
        for table_name in &tables[1..] {
            let table = self.tables.get(table_name)?;
            if !first_table.is_colocated_with(table) {
                return None;
            }
        }

        // Get distribution column and value
        let dist_col = match &first_table.strategy {
            DistributionStrategy::Hash { column } => column,
            _ => return None,
        };

        let value = filter_values.get(dist_col)?;
        let shard = first_table.get_shard_for_value(value)?;

        Some(DistributedPlan::Router {
            worker_id: shard.worker_id.clone(),
            local_plan: Box::new(plan.clone()),
        })
    }

    /// Check if query can be fully pushed down to workers
    fn can_pushdown_fully(&self, plan: &PlanNode) -> bool {
        // Can pushdown if:
        // - No cross-shard joins
        // - All aggregations are parallelizable
        // For now, simplified: allow scans and simple aggregations
        matches!(plan, PlanNode::Scan { .. } | PlanNode::Aggregate { .. })
    }

    /// Plan parallel pushdown: send query to all workers
    fn plan_parallel_pushdown(&self, plan: &PlanNode) -> DistributedPlan {
        let mut worker_plans = HashMap::new();

        // Send same plan to all workers
        for worker in &self.workers {
            worker_plans.insert(worker.id.clone(), plan.clone());
        }

        // Coordinator aggregates results
        let coordinator_plan = Some(Box::new(PlanNode::Aggregate {
            input: Box::new(plan.clone()),
            group_by: vec![],
            aggregates: vec!["SUM".to_string()],
            cost: PlanCost::default(),
        }));

        DistributedPlan::Parallel {
            worker_plans,
            coordinator_plan,
        }
    }

    /// Plan repartition join: shuffle data for non-co-located joins
    fn plan_repartition_join(&self, plan: &PlanNode) -> DistributedPlan {
        // Simplified: broadcast smaller table to all workers
        match plan {
            PlanNode::Join { left, right, .. } => {
                // Check which side is smaller
                let left_cost = self.estimate_size(left);
                let right_cost = self.estimate_size(right);

                let broadcast_table = if left_cost < right_cost {
                    self.extract_tables(left).first().unwrap().clone()
                } else {
                    self.extract_tables(right).first().unwrap().clone()
                };

                let mut worker_plans = HashMap::new();
                for worker in &self.workers {
                    worker_plans.insert(worker.id.clone(), plan.clone());
                }

                DistributedPlan::BroadcastJoin {
                    broadcast_table,
                    worker_plans,
                }
            }
            _ => {
                // Fallback to parallel pushdown
                self.plan_parallel_pushdown(plan)
            }
        }
    }

    /// Extract table names from plan
    fn extract_tables(&self, plan: &PlanNode) -> Vec<String> {
        match plan {
            PlanNode::Scan { table, .. } => vec![table.clone()],
            PlanNode::Join { left, right, .. } => {
                let mut tables = self.extract_tables(left);
                tables.extend(self.extract_tables(right));
                tables
            }
            PlanNode::Aggregate { input, .. } => self.extract_tables(input),
            PlanNode::Sort { input, .. } => self.extract_tables(input),
            PlanNode::Project { input, .. } => self.extract_tables(input),
        }
    }

    /// Estimate plan size (for broadcast decision)
    fn estimate_size(&self, plan: &PlanNode) -> u64 {
        match plan {
            PlanNode::Scan { cost, .. } => cost.estimated_rows,
            PlanNode::Join { cost, .. } => cost.estimated_rows,
            PlanNode::Aggregate { cost, .. } => cost.estimated_rows,
            PlanNode::Sort { cost, .. } => cost.estimated_rows,
            PlanNode::Project { cost, .. } => cost.estimated_rows,
        }
    }

    /// Get recommended planner type for a query
    pub fn choose_planner(
        &self,
        plan: &PlanNode,
        filter_values: &HashMap<String, Value>,
    ) -> PlannerType {
        if self.try_fast_path(plan, filter_values).is_some() {
            PlannerType::FastPath
        } else if self.try_router_planner(plan, filter_values).is_some() {
            PlannerType::Router
        } else if self.can_pushdown_fully(plan) {
            PlannerType::LogicalPushdown
        } else {
            PlannerType::LogicalJoinOrder
        }
    }
}

impl Default for DistributedPlanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::advanced_optimizer::ScanMethod;

    #[test]
    fn test_hash_distribution() {
        let shards = vec![
            ShardInfo {
                id: 1,
                worker_id: "worker1".to_string(),
                range: None,
                row_count: 1000,
            },
            ShardInfo {
                id: 2,
                worker_id: "worker2".to_string(),
                range: None,
                row_count: 1000,
            },
        ];

        let table = DistributedTable {
            name: "users".to_string(),
            strategy: DistributionStrategy::Hash {
                column: "id".to_string(),
            },
            shards,
        };

        let value = Value::Integer(42);
        let shard = table.get_shard_for_value(&value);
        assert!(shard.is_some());
    }

    #[test]
    fn test_colocated_tables() {
        let table1 = DistributedTable {
            name: "users".to_string(),
            strategy: DistributionStrategy::Hash {
                column: "id".to_string(),
            },
            shards: vec![ShardInfo {
                id: 1,
                worker_id: "worker1".to_string(),
                range: None,
                row_count: 1000,
            }],
        };

        let table2 = DistributedTable {
            name: "orders".to_string(),
            strategy: DistributionStrategy::Hash {
                column: "id".to_string(),
            },
            shards: vec![ShardInfo {
                id: 2,
                worker_id: "worker1".to_string(),
                range: None,
                row_count: 2000,
            }],
        };

        assert!(table1.is_colocated_with(&table2));
    }

    #[test]
    fn test_fast_path_planner() {
        let mut planner = DistributedPlanner::new();

        let table = DistributedTable {
            name: "users".to_string(),
            strategy: DistributionStrategy::Hash {
                column: "id".to_string(),
            },
            shards: vec![ShardInfo {
                id: 1,
                worker_id: "worker1".to_string(),
                range: None,
                row_count: 1000,
            }],
        };
        planner.register_table(table);

        let plan = PlanNode::Scan {
            table: "users".to_string(),
            method: ScanMethod::Sequential,
            filter: Some("id = 42".to_string()),
            cost: PlanCost::default(),
        };

        let mut filter_values = HashMap::new();
        filter_values.insert("id".to_string(), Value::Integer(42));

        let dist_plan = planner.try_fast_path(&plan, &filter_values);
        assert!(dist_plan.is_some());

        match dist_plan.unwrap() {
            DistributedPlan::Router { worker_id, .. } => {
                assert_eq!(worker_id, "worker1");
            }
            _ => panic!("Expected router plan"),
        }
    }

    #[test]
    fn test_planner_type_selection() {
        let mut planner = DistributedPlanner::new();

        let table = DistributedTable {
            name: "users".to_string(),
            strategy: DistributionStrategy::Hash {
                column: "id".to_string(),
            },
            shards: vec![ShardInfo {
                id: 1,
                worker_id: "worker1".to_string(),
                range: None,
                row_count: 1000,
            }],
        };
        planner.register_table(table);

        let plan = PlanNode::Scan {
            table: "users".to_string(),
            method: ScanMethod::Sequential,
            filter: Some("id = 42".to_string()),
            cost: PlanCost::default(),
        };

        let mut filter_values = HashMap::new();
        filter_values.insert("id".to_string(), Value::Integer(42));

        let planner_type = planner.choose_planner(&plan, &filter_values);
        assert_eq!(planner_type, PlannerType::FastPath);
    }
}
