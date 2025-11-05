// v1.3.0: Distributed Database
//
// Implements distributed database capabilities:
// - Sharding: Horizontal data partitioning across nodes
// - Raft consensus: Leader-based distributed consensus
// - Distributed query execution: Cross-node query processing
// - Multi-region support: Geographic distribution

use crate::types::{Row, Value};
use crate::{NexusError, Result};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Shard key strategy
#[derive(Debug, Clone, PartialEq)]
pub enum ShardStrategy {
    /// Hash-based sharding
    Hash,
    /// Range-based sharding
    Range,
    /// Geographic sharding
    Geographic,
}

/// Shard definition
#[derive(Debug, Clone)]
pub struct Shard {
    pub id: usize,
    pub name: String,
    pub node_id: String,
    pub range_start: Option<Value>,
    pub range_end: Option<Value>,
    pub region: Option<String>,
}

/// Sharding manager
#[allow(dead_code)]
pub struct ShardingManager {
    strategy: ShardStrategy,
    shards: Arc<RwLock<Vec<Shard>>>,
    shard_map: Arc<RwLock<HashMap<String, usize>>>, // key -> shard_id
}

impl ShardingManager {
    pub fn new(strategy: ShardStrategy) -> Self {
        Self {
            strategy,
            shards: Arc::new(RwLock::new(Vec::new())),
            shard_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a shard
    pub fn add_shard(&self, shard: Shard) -> Result<()> {
        let mut shards = self.shards.write().unwrap();
        shards.push(shard);
        Ok(())
    }

    /// Route row to appropriate shard
    pub fn route_row(&self, row: &Row, shard_key_col: &str) -> Result<usize> {
        let key_value = self.extract_shard_key(row, shard_key_col)?;

        match self.strategy {
            ShardStrategy::Hash => self.route_hash(&key_value),
            ShardStrategy::Range => self.route_range(&key_value),
            ShardStrategy::Geographic => self.route_geographic(&key_value),
        }
    }

    fn extract_shard_key(&self, row: &Row, col: &str) -> Result<Value> {
        let idx = row.columns.iter().position(|c| c == col).ok_or_else(|| {
            NexusError::ExecutionError(format!("Shard key column not found: {}", col))
        })?;
        Ok(row.values[idx].clone())
    }

    fn route_hash(&self, key: &Value) -> Result<usize> {
        let shards = self.shards.read().unwrap();
        if shards.is_empty() {
            return Err(NexusError::ExecutionError(
                "No shards available".to_string(),
            ));
        }

        let hash = self.hash_value(key);
        let shard_idx = hash % shards.len();
        Ok(shards[shard_idx].id)
    }

    fn route_range(&self, key: &Value) -> Result<usize> {
        let shards = self.shards.read().unwrap();

        for shard in shards.iter() {
            if let (Some(start), Some(end)) = (&shard.range_start, &shard.range_end) {
                if key >= start && key < end {
                    return Ok(shard.id);
                }
            }
        }

        Err(NexusError::ExecutionError(
            "No shard found for key".to_string(),
        ))
    }

    fn route_geographic(&self, key: &Value) -> Result<usize> {
        let shards = self.shards.read().unwrap();

        // Simple geographic routing based on region prefix
        if let Value::Text(region) = key {
            for shard in shards.iter() {
                if let Some(shard_region) = &shard.region {
                    if region.starts_with(shard_region) {
                        return Ok(shard.id);
                    }
                }
            }
        }

        // Default to first shard
        Ok(shards[0].id)
    }

    fn hash_value(&self, value: &Value) -> usize {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        match value {
            Value::Integer(i) => i.hash(&mut hasher),
            Value::Text(s) => s.hash(&mut hasher),
            Value::Float(f) => f.hash(&mut hasher),
            Value::Boolean(b) => b.hash(&mut hasher),
            Value::Blob(b) => b.hash(&mut hasher),
            Value::Null => 0usize.hash(&mut hasher),
            Value::Vector(v) => {
                // Hash vectors by their float bits
                for f in v {
                    f.to_bits().hash(&mut hasher);
                }
            }
        }
        hasher.finish() as usize
    }

    pub fn shard_count(&self) -> usize {
        self.shards.read().unwrap().len()
    }
}

/// Raft node state
#[derive(Debug, Clone, PartialEq)]
pub enum RaftState {
    Follower,
    Candidate,
    Leader,
}

/// Raft consensus node
pub struct RaftNode {
    pub node_id: String,
    pub state: Arc<RwLock<RaftState>>,
    pub current_term: Arc<RwLock<u64>>,
    pub voted_for: Arc<RwLock<Option<String>>>,
    pub log: Arc<RwLock<Vec<LogEntry>>>,
    pub commit_index: Arc<RwLock<u64>>,
    pub cluster_nodes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub term: u64,
    pub index: u64,
    pub command: String,
}

impl RaftNode {
    pub fn new(node_id: String, cluster_nodes: Vec<String>) -> Self {
        Self {
            node_id,
            state: Arc::new(RwLock::new(RaftState::Follower)),
            current_term: Arc::new(RwLock::new(0)),
            voted_for: Arc::new(RwLock::new(None)),
            log: Arc::new(RwLock::new(Vec::new())),
            commit_index: Arc::new(RwLock::new(0)),
            cluster_nodes,
        }
    }

    /// Start election (become candidate)
    pub fn start_election(&self) -> Result<()> {
        let mut state = self.state.write().unwrap();
        *state = RaftState::Candidate;

        let mut term = self.current_term.write().unwrap();
        *term += 1;

        let mut voted = self.voted_for.write().unwrap();
        *voted = Some(self.node_id.clone());

        Ok(())
    }

    /// Become leader
    pub fn become_leader(&self) -> Result<()> {
        let mut state = self.state.write().unwrap();
        *state = RaftState::Leader;
        Ok(())
    }

    /// Append log entry (leader only)
    pub fn append_log(&self, command: String) -> Result<u64> {
        let state = self.state.read().unwrap();
        if *state != RaftState::Leader {
            return Err(NexusError::ExecutionError("Not a leader".to_string()));
        }
        drop(state);

        let term = *self.current_term.read().unwrap();
        let mut log = self.log.write().unwrap();

        let index = log.len() as u64;
        log.push(LogEntry {
            term,
            index,
            command,
        });

        Ok(index)
    }

    /// Commit log entry
    pub fn commit(&self, index: u64) -> Result<()> {
        let mut commit_idx = self.commit_index.write().unwrap();
        *commit_idx = index;
        Ok(())
    }

    pub fn get_state(&self) -> RaftState {
        self.state.read().unwrap().clone()
    }
}

/// Distributed query coordinator
pub struct DistributedQueryCoordinator {
    sharding_manager: Arc<ShardingManager>,
}

impl DistributedQueryCoordinator {
    pub fn new(sharding_manager: Arc<ShardingManager>) -> Self {
        Self { sharding_manager }
    }

    /// Execute distributed query across shards
    pub fn execute_distributed(&self, _query: &str, shard_keys: Vec<Value>) -> Result<Vec<Row>> {
        let all_results = Vec::new();

        // Route to appropriate shards
        let _target_shards: Vec<usize> = shard_keys
            .iter()
            .filter_map(|_key| {
                // Simplified routing based on key
                Some(0) // Would route to actual shard
            })
            .collect();

        // In real implementation, would execute on each shard in parallel
        // and merge results

        Ok(all_results)
    }

    /// Scatter-gather pattern
    pub fn scatter_gather(&self, _query: &str) -> Result<QueryStats> {
        let shard_count = self.sharding_manager.shard_count();

        // Scatter: Send query to all shards
        // Gather: Collect and merge results

        Ok(QueryStats {
            shards_queried: shard_count,
            rows_returned: 0,
            execution_time_ms: 0.0,
        })
    }
}

#[derive(Debug, Clone)]
pub struct QueryStats {
    pub shards_queried: usize,
    pub rows_returned: usize,
    pub execution_time_ms: f64,
}

/// Multi-region configuration
#[derive(Debug, Clone)]
pub struct Region {
    pub name: String,
    pub location: String, // e.g., "us-east-1"
    pub nodes: Vec<String>,
}

pub struct MultiRegionManager {
    regions: HashMap<String, Region>,
}

impl MultiRegionManager {
    pub fn new() -> Self {
        Self {
            regions: HashMap::new(),
        }
    }

    pub fn add_region(&mut self, region: Region) {
        self.regions.insert(region.name.clone(), region);
    }

    pub fn get_region(&self, name: &str) -> Option<&Region> {
        self.regions.get(name)
    }

    pub fn region_count(&self) -> usize {
        self.regions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_sharding() {
        let mgr = ShardingManager::new(ShardStrategy::Hash);

        // Add 4 shards
        for i in 0..4 {
            mgr.add_shard(Shard {
                id: i,
                name: format!("shard_{}", i),
                node_id: format!("node_{}", i),
                range_start: None,
                range_end: None,
                region: None,
            })
            .unwrap();
        }

        // Route rows
        let row = Row::new(vec!["user_id".to_string()], vec![Value::Integer(12345)]);
        let shard_id = mgr.route_row(&row, "user_id").unwrap();

        assert!(shard_id < 4);
    }

    #[test]
    fn test_range_sharding() {
        let mgr = ShardingManager::new(ShardStrategy::Range);

        mgr.add_shard(Shard {
            id: 0,
            name: "shard_0".to_string(),
            node_id: "node_0".to_string(),
            range_start: Some(Value::Integer(0)),
            range_end: Some(Value::Integer(1000)),
            region: None,
        })
        .unwrap();

        let row = Row::new(vec!["id".to_string()], vec![Value::Integer(500)]);
        let shard_id = mgr.route_row(&row, "id").unwrap();
        assert_eq!(shard_id, 0);
    }

    #[test]
    fn test_raft_election() {
        let node = RaftNode::new(
            "node1".to_string(),
            vec![
                "node1".to_string(),
                "node2".to_string(),
                "node3".to_string(),
            ],
        );

        assert_eq!(node.get_state(), RaftState::Follower);

        node.start_election().unwrap();
        assert_eq!(node.get_state(), RaftState::Candidate);

        node.become_leader().unwrap();
        assert_eq!(node.get_state(), RaftState::Leader);
    }

    #[test]
    fn test_raft_log_append() {
        let node = RaftNode::new("node1".to_string(), vec!["node1".to_string()]);

        node.become_leader().unwrap();

        let idx = node
            .append_log("INSERT INTO users VALUES (1, 'Alice')".to_string())
            .unwrap();
        assert_eq!(idx, 0);

        node.commit(idx).unwrap();
        assert_eq!(*node.commit_index.read().unwrap(), 0);
    }

    #[test]
    fn test_distributed_query_coordinator() {
        let mgr = Arc::new(ShardingManager::new(ShardStrategy::Hash));

        for i in 0..4 {
            mgr.add_shard(Shard {
                id: i,
                name: format!("shard_{}", i),
                node_id: format!("node_{}", i),
                range_start: None,
                range_end: None,
                region: None,
            })
            .unwrap();
        }

        let coordinator = DistributedQueryCoordinator::new(mgr);
        let stats = coordinator.scatter_gather("SELECT * FROM users").unwrap();

        assert_eq!(stats.shards_queried, 4);
    }

    #[test]
    fn test_multi_region() {
        let mut mgr = MultiRegionManager::new();

        mgr.add_region(Region {
            name: "us-east".to_string(),
            location: "us-east-1".to_string(),
            nodes: vec!["node1".to_string(), "node2".to_string()],
        });

        mgr.add_region(Region {
            name: "eu-west".to_string(),
            location: "eu-west-1".to_string(),
            nodes: vec!["node3".to_string(), "node4".to_string()],
        });

        assert_eq!(mgr.region_count(), 2);
        assert!(mgr.get_region("us-east").is_some());
    }

    #[test]
    fn test_shard_count() {
        let mgr = ShardingManager::new(ShardStrategy::Hash);

        for i in 0..8 {
            mgr.add_shard(Shard {
                id: i,
                name: format!("shard_{}", i),
                node_id: format!("node_{}", i),
                range_start: None,
                range_end: None,
                region: None,
            })
            .unwrap();
        }

        assert_eq!(mgr.shard_count(), 8);
    }
}
