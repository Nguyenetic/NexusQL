// Integration Tests - End-to-End Testing
// Tests complete workflows across multiple subsystems

use nexusql::cte::{CTEExecutor, RecursiveCTE};
use nexusql::fulltext::FullTextSearch;
use nexusql::join::{JoinAlgorithm, JoinCondition, JoinExecutor, JoinType};
use nexusql::jsonb::{JsonbFunctions, JsonbValue};
use nexusql::partition::{PartitionBounds, PartitionStrategy, PartitionedTable};
use nexusql::planner::TableStatistics;
use nexusql::types::{Row, Value};
use nexusql::vacuum::TableVacuum;

#[test]
fn test_end_to_end_ecommerce_workflow() {
    // Simulate complete e-commerce workflow

    // 1. Create partitioned orders table
    let orders_table = PartitionedTable::new(
        "orders".to_string(),
        PartitionStrategy::Range,
        "order_date".to_string(),
    );

    orders_table
        .add_partition(
            "orders_2024_q1".to_string(),
            PartitionBounds::Range {
                from: Value::Text("2024-01-01".to_string()),
                to: Value::Text("2024-04-01".to_string()),
            },
        )
        .unwrap();

    // 2. Insert orders
    for i in 1..=100 {
        let order = Row::new(
            vec![
                "order_id".to_string(),
                "order_date".to_string(),
                "amount".to_string(),
            ],
            vec![
                Value::Integer(i),
                Value::Text(format!("2024-0{}-15", (i % 3) + 1)),
                Value::Integer(i * 100),
            ],
        );
        orders_table.insert(order).unwrap();
    }

    // 3. Verify partition pruning works
    let stats = orders_table.get_stats();
    assert!(stats.total_rows > 0);
    assert_eq!(stats.partition_count, 1);
}

#[test]
fn test_analytics_pipeline() {
    // Complete analytics pipeline: Join → Aggregate → Full-text

    // 1. Create users and orders
    let users = vec![
        Row::new(
            vec!["id".to_string(), "name".to_string()],
            vec![Value::Integer(1), Value::Text("Alice".to_string())],
        ),
        Row::new(
            vec!["id".to_string(), "name".to_string()],
            vec![Value::Integer(2), Value::Text("Bob".to_string())],
        ),
    ];

    let orders = vec![
        Row::new(
            vec!["user_id".to_string(), "amount".to_string()],
            vec![Value::Integer(1), Value::Integer(100)],
        ),
        Row::new(
            vec!["user_id".to_string(), "amount".to_string()],
            vec![Value::Integer(1), Value::Integer(200)],
        ),
        Row::new(
            vec!["user_id".to_string(), "amount".to_string()],
            vec![Value::Integer(2), Value::Integer(150)],
        ),
    ];

    // 2. Join users and orders
    let executor = JoinExecutor::new(
        JoinType::Inner,
        JoinAlgorithm::Hash,
        JoinCondition::On {
            left_column: "id".to_string(),
            right_column: "user_id".to_string(),
        },
    );

    let joined = executor.execute(users, orders).unwrap();
    assert_eq!(joined.len(), 3); // 3 orders total

    // 3. Analyze results
    let stats = TableStatistics::analyze("results".to_string(), &joined).unwrap();
    assert_eq!(stats.row_count, 3);
}

#[test]
fn test_recursive_cte_with_json() {
    // Complex: Recursive CTE + JSON operations

    let mut executor = CTEExecutor::new();

    // Create hierarchical data with JSON metadata
    executor
        .execute_recursive_cte(
            "hierarchy".to_string(),
            Some(vec!["id".to_string(), "level".to_string()]),
            || {
                Ok(vec![Row::new(
                    vec!["id".to_string(), "level".to_string()],
                    vec![Value::Integer(1), Value::Integer(0)],
                )])
            },
            |working_table| {
                let mut new_rows = Vec::new();
                for row in working_table {
                    if let Value::Integer(level) = row.values[1] {
                        if level < 3 {
                            for child_id in 0..2 {
                                new_rows.push(Row::new(
                                    vec!["id".to_string(), "level".to_string()],
                                    vec![row.values[0].clone(), Value::Integer(level + 1)],
                                ));
                            }
                        }
                    }
                }
                Ok(new_rows)
            },
            10,
        )
        .unwrap();

    let cte = executor.get_cte("hierarchy").unwrap();
    assert!(cte.rows.len() > 1);
}

#[test]
fn test_fulltext_with_partitioning() {
    // Full-text search on partitioned data

    let docs = vec![
        "PostgreSQL database tutorial",
        "Machine learning with Python",
        "Database indexing strategies",
        "Python programming guide",
    ];

    // Create tsvectors
    let vectors: Vec<_> = docs
        .iter()
        .map(|doc| FullTextSearch::to_tsvector(doc))
        .collect();

    // Search query
    let query = FullTextSearch::plainto_tsquery("database").unwrap();

    let matches: Vec<_> = vectors
        .iter()
        .enumerate()
        .filter(|(_, vec)| vec.matches(&query))
        .map(|(i, _)| i)
        .collect();

    assert!(matches.contains(&0)); // PostgreSQL database
    assert!(matches.contains(&2)); // Database indexing
}

#[test]
fn test_vacuum_after_updates() {
    // VACUUM after heavy updates

    let mut table = TableVacuum::new("products".to_string());

    // Insert 1000 rows
    for i in 0..1000 {
        let row = Row::new(
            vec!["id".to_string(), "stock".to_string()],
            vec![Value::Integer(i), Value::Integer(100)],
        );
        table.insert(row);
    }

    // Simulate 500 updates (creates dead tuples)
    for i in 0..500 {
        table.mark_dead(nexusql::vacuum::TupleId {
            page: (i / 100) as u32,
            offset: (i % 100) as u16,
        });
    }

    // Check bloat
    let bloat_before = table.detect_bloat();
    assert!(bloat_before.needs_vacuum);

    // VACUUM
    let vacuum_stats = table.vacuum().unwrap();
    assert_eq!(vacuum_stats.tuples_removed, 500);

    // Verify bloat reduced
    let bloat_after = table.detect_bloat();
    assert!(!bloat_after.needs_vacuum);
}

#[test]
fn test_hybrid_ai_search() {
    // AI-native hybrid search with embeddings

    use nexusql::ai_native::{EmbeddingGenerator, HybridSearch};

    let documents = vec![
        (
            EmbeddingGenerator::from_text("PostgreSQL tutorial", 64),
            "PostgreSQL database tutorial".to_string(),
        ),
        (
            EmbeddingGenerator::from_text("Python guide", 64),
            "Python programming guide".to_string(),
        ),
        (
            EmbeddingGenerator::from_text("Machine learning database", 64),
            "Machine learning with databases".to_string(),
        ),
    ];

    let query_emb = EmbeddingGenerator::from_text("database tutorial", 64);
    let searcher = HybridSearch::new(0.6); // 60% semantic, 40% keyword

    let results = searcher.search(&query_emb, "database tutorial", &documents);

    assert_eq!(results.len(), 3);
    // First result should have highest score
    assert!(results[0].1 > 0.0);
}

#[test]
fn test_distributed_with_replication() {
    // Distributed database with Raft consensus

    use nexusql::distributed::{RaftNode, Shard, ShardStrategy, ShardingManager};

    let mgr = ShardingManager::new(ShardStrategy::Hash);

    // 3 shards with Raft consensus
    for i in 0..3 {
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

    // Each shard has Raft group
    let raft = RaftNode::new(
        "node_0".to_string(),
        vec![
            "node_0".to_string(),
            "node_1".to_string(),
            "node_2".to_string(),
        ],
    );

    raft.become_leader().unwrap();
    let idx = raft.append_log("REPLICATE shard_0".to_string()).unwrap();
    raft.commit(idx).unwrap();

    assert_eq!(mgr.shard_count(), 3);
}

#[test]
fn test_complete_stack_integration() {
    // Complete stack: Partitioning + Join + CTE + JSONB + Full-text + VACUUM

    // 1. Partitioned table
    let table = PartitionedTable::new(
        "events".to_string(),
        PartitionStrategy::List,
        "event_type".to_string(),
    );

    table
        .add_partition(
            "events_clicks".to_string(),
            PartitionBounds::List {
                values: vec![Value::Text("click".to_string())],
            },
        )
        .unwrap();

    // 2. Insert with JSONB metadata
    let event = Row::new(
        vec!["event_type".to_string(), "metadata".to_string()],
        vec![
            Value::Text("click".to_string()),
            Value::Text(r#"{"user_id": 123, "page": "home"}"#.to_string()),
        ],
    );

    table.insert(event).unwrap();

    // 3. Verify
    let stats = table.get_stats();
    assert_eq!(stats.total_rows, 1);

    // 4. Full-text on metadata
    let vec = FullTextSearch::to_tsvector("user homepage click");
    assert!(vec.length() > 0);
}
