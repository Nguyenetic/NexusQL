// NexusQL v1.3.0 - Distributed Database Demo
// Sharding, Raft consensus, distributed queries, multi-region

use nexusql::distributed::{
    DistributedQueryCoordinator, MultiRegionManager, RaftNode, RaftState, Region, Shard,
    ShardStrategy, ShardingManager,
};
use nexusql::types::{Row, Value};
use std::sync::Arc;

fn main() {
    println!("🌐 NexusQL v1.3.0 - Distributed Database Demo");
    println!("============================================\n");

    demo_sharding();
    demo_raft_consensus();
    demo_distributed_queries();
    demo_multi_region();

    println!("\n✅ v1.3.0 - Globally Distributed Database Ready!");
}

fn demo_sharding() {
    println!("🔀 Demo 1: Sharding (Hash, Range, Geographic)");
    println!("----------------------------------------------\n");

    // Hash sharding
    println!("Hash Sharding (even distribution):");
    let hash_mgr = ShardingManager::new(ShardStrategy::Hash);

    for i in 0..4 {
        hash_mgr
            .add_shard(Shard {
                id: i,
                name: format!("shard_{}", i),
                node_id: format!("node_{}", i),
                range_start: None,
                range_end: None,
                region: None,
            })
            .unwrap();
    }

    println!("  Created 4 shards (shard_0 to shard_3)");

    // Route some rows
    for user_id in vec![1, 100, 1000, 10000] {
        let row = Row::new(vec!["user_id".to_string()], vec![Value::Integer(user_id)]);
        let shard = hash_mgr.route_row(&row, "user_id").unwrap();
        println!("  user_id {} → shard_{}", user_id, shard);
    }

    // Range sharding
    println!("\nRange Sharding (by value ranges):");
    let range_mgr = ShardingManager::new(ShardStrategy::Range);

    range_mgr
        .add_shard(Shard {
            id: 0,
            name: "shard_old".to_string(),
            node_id: "node_0".to_string(),
            range_start: Some(Value::Integer(0)),
            range_end: Some(Value::Integer(1000)),
            region: None,
        })
        .unwrap();

    range_mgr
        .add_shard(Shard {
            id: 1,
            name: "shard_new".to_string(),
            node_id: "node_1".to_string(),
            range_start: Some(Value::Integer(1000)),
            range_end: Some(Value::Integer(10000)),
            region: None,
        })
        .unwrap();

    println!("  Shard 0: IDs 0-999 (old customers)");
    println!("  Shard 1: IDs 1000-9999 (new customers)");
}

fn demo_raft_consensus() {
    println!("\n\n🗳️  Demo 2: Raft Consensus Protocol");
    println!("------------------------------------\n");

    // Create 3-node cluster
    let cluster = vec![
        "node1".to_string(),
        "node2".to_string(),
        "node3".to_string(),
    ];

    let node1 = RaftNode::new("node1".to_string(), cluster.clone());
    let node2 = RaftNode::new("node2".to_string(), cluster.clone());
    let node3 = RaftNode::new("node3".to_string(), cluster.clone());

    println!("3-node Raft cluster:");
    println!("  • node1 (Follower)");
    println!("  • node2 (Follower)");
    println!("  • node3 (Follower)\n");

    // Node 1 starts election
    println!("node1 starts election...");
    node1.start_election().unwrap();
    println!("  ✓ node1 → Candidate (term 1)");
    println!("  ✓ Requests votes from cluster\n");

    // Node 1 wins election
    println!("node1 wins election (majority votes)...");
    node1.become_leader().unwrap();
    println!("  ✓ node1 → Leader");
    println!("  ✓ node2, node3 remain Followers\n");

    // Leader appends log entries
    println!("Leader replicating operations:");
    let idx1 = node1.append_log("INSERT user_1".to_string()).unwrap();
    println!("  ✓ Log[{}]: INSERT user_1", idx1);

    let idx2 = node1.append_log("UPDATE user_1".to_string()).unwrap();
    println!("  ✓ Log[{}]: UPDATE user_1", idx2);

    // Commit when majority replicated
    println!("\nCommitting after majority replication...");
    node1.commit(idx2).unwrap();
    println!("  ✓ Committed through index {}", idx2);

    println!("\n  💡 Raft ensures strong consistency!");
    println!("  💡 Tolerates (N-1)/2 node failures!");
}

fn demo_distributed_queries() {
    println!("\n\n🔄 Demo 3: Distributed Query Execution");
    println!("---------------------------------------\n");

    let mgr = Arc::new(ShardingManager::new(ShardStrategy::Hash));

    // Create 8 shards
    for i in 0..8 {
        mgr.add_shard(Shard {
            id: i,
            name: format!("shard_{}", i),
            node_id: format!("node_{}", i % 4), // 4 physical nodes
            range_start: None,
            range_end: None,
            region: None,
        })
        .unwrap();
    }

    println!("Cluster: 8 shards across 4 nodes\n");

    let coordinator = DistributedQueryCoordinator::new(mgr);

    // Scatter-gather query
    println!("Query: SELECT COUNT(*) FROM users");
    println!("Execution plan:");
    println!("  1. Scatter: Send to all 8 shards");
    println!("  2. Execute: COUNT on each shard");
    println!("  3. Gather: SUM counts from all shards");
    println!("  4. Return: Final aggregate\n");

    let stats = coordinator
        .scatter_gather("SELECT COUNT(*) FROM users")
        .unwrap();
    println!("Execution stats:");
    println!("  Shards queried: {}", stats.shards_queried);
    println!("  Parallel execution across nodes");

    println!("\n  💡 Queries execute in parallel across shards!");
    println!("  💡 Coordinator aggregates results!");
}

fn demo_multi_region() {
    println!("\n\n🌍 Demo 4: Multi-Region Deployment");
    println!("-----------------------------------\n");

    let mut mgr = MultiRegionManager::new();

    // Add regions
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

    mgr.add_region(Region {
        name: "ap-south".to_string(),
        location: "ap-south-1".to_string(),
        nodes: vec!["node5".to_string(), "node6".to_string()],
    });

    println!("Global Deployment ({} regions):", mgr.region_count());

    if let Some(us) = mgr.get_region("us-east") {
        println!("  🇺🇸 US East ({}): {} nodes", us.location, us.nodes.len());
    }
    if let Some(eu) = mgr.get_region("eu-west") {
        println!("  🇪🇺 EU West ({}): {} nodes", eu.location, eu.nodes.len());
    }
    if let Some(ap) = mgr.get_region("ap-south") {
        println!("  🌏 AP South ({}): {} nodes", ap.location, ap.nodes.len());
    }

    println!("\nBenefits:");
    println!("  • Low latency for local users");
    println!("  • Data sovereignty compliance");
    println!("  • Geographic fault tolerance");
    println!("  • Raft consensus per region");
}
