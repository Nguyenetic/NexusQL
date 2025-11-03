//! Join Order Optimization - Week 3-4
//!
//! Context7 Best Practices from PostgreSQL:
//! - Dynamic programming for optimal join order (below threshold)
//! - Genetic algorithm for large join graphs (GEQO)
//! - Bushy join trees, not just left-deep
//! - Cost-based selection of join algorithms
//!
//! PROPRIETARY & PATENTABLE

use crate::engine::advanced_optimizer::{AdvancedOptimizer, JoinAlgorithm, PlanCost, PlanNode};
use std::collections::{HashMap, HashSet};

/// Join relation (table or subquery result)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Relation {
    pub name: String,
    pub alias: Option<String>,
}

/// Join condition between two relations
#[derive(Debug, Clone)]
pub struct JoinCondition {
    pub left_table: String,
    pub right_table: String,
    pub left_column: String,
    pub right_column: String,
    pub condition: String,
}

/// Join graph representing all possible joins
#[derive(Debug)]
pub struct JoinGraph {
    /// All relations in the query
    relations: Vec<Relation>,
    /// Join conditions between relations
    conditions: Vec<JoinCondition>,
    /// Which relations can be joined
    adjacency: HashMap<String, HashSet<String>>,
}

impl JoinGraph {
    pub fn new(relations: Vec<Relation>, conditions: Vec<JoinCondition>) -> Self {
        let mut adjacency = HashMap::new();

        // Build adjacency list
        for cond in &conditions {
            adjacency
                .entry(cond.left_table.clone())
                .or_insert_with(HashSet::new)
                .insert(cond.right_table.clone());
            adjacency
                .entry(cond.right_table.clone())
                .or_insert_with(HashSet::new)
                .insert(cond.left_table.clone());
        }

        Self {
            relations,
            conditions,
            adjacency,
        }
    }

    /// Check if two relations can be joined
    pub fn can_join(&self, left: &str, right: &str) -> bool {
        self.adjacency
            .get(left)
            .map(|adj| adj.contains(right))
            .unwrap_or(false)
    }

    /// Get join condition between two relations
    pub fn get_join_condition(&self, left: &str, right: &str) -> Option<&JoinCondition> {
        self.conditions.iter().find(|c| {
            (c.left_table == left && c.right_table == right)
                || (c.left_table == right && c.right_table == left)
        })
    }
}

/// Join order optimizer using dynamic programming
pub struct JoinOrderOptimizer {
    /// Reference to cost estimator
    optimizer: AdvancedOptimizer,
    /// Threshold for switching to GEQO (PostgreSQL default: 12)
    geqo_threshold: usize,
    /// Memoization cache for dynamic programming
    memo: HashMap<Vec<String>, (PlanNode, PlanCost)>,
}

impl JoinOrderOptimizer {
    pub fn new(optimizer: AdvancedOptimizer) -> Self {
        Self {
            optimizer,
            geqo_threshold: 12,
            memo: HashMap::new(),
        }
    }

    /// Find optimal join order using dynamic programming
    ///
    /// PostgreSQL approach:
    /// 1. For each subset size from 1 to N
    /// 2. For each subset of that size
    /// 3. Try splitting it into two smaller subsets
    /// 4. Pick the lowest cost join
    pub fn optimize_join_order(&mut self, graph: &JoinGraph) -> PlanNode {
        let n = graph.relations.len();

        if n == 0 {
            panic!("No relations to join");
        }

        if n == 1 {
            // Single table - just scan it
            let table = &graph.relations[0].name;
            return self.create_scan_plan(table);
        }

        if n > self.geqo_threshold {
            // Use genetic algorithm for large queries
            return self.optimize_with_geqo(graph);
        }

        // Use dynamic programming for small queries
        self.optimize_with_dp(graph)
    }

    /// Dynamic programming join order optimization
    fn optimize_with_dp(&mut self, graph: &JoinGraph) -> PlanNode {
        let n = graph.relations.len();
        let relation_names: Vec<String> = graph.relations.iter().map(|r| r.name.clone()).collect();

        // Base case: single relations
        for table in &relation_names {
            let plan = self.create_scan_plan(table);
            let cost = self.optimizer.get_plan_cost(&plan);
            self.memo.insert(vec![table.clone()], (plan, cost));
        }

        // Build up larger join sets
        for size in 2..=n {
            // Generate all subsets of size `size`
            for subset in Self::generate_subsets(&relation_names, size) {
                let best = self.find_best_join_for_subset(graph, &subset);
                self.memo.insert(subset, best);
            }
        }

        // Return the best plan for all relations
        let (plan, _) = self.memo.get(&relation_names).unwrap().clone();
        plan
    }

    /// Find best way to join a subset of relations
    fn find_best_join_for_subset(
        &self,
        graph: &JoinGraph,
        subset: &[String],
    ) -> (PlanNode, PlanCost) {
        let n = subset.len();
        let mut best_plan = None;
        let mut best_cost = PlanCost {
            total_cost: f64::INFINITY,
            ..Default::default()
        };

        // Try all ways to split subset into two parts
        for left_size in 1..n {
            for left_subset in Self::generate_subsets(subset, left_size) {
                // Right subset = subset - left subset
                let right_subset: Vec<String> = subset
                    .iter()
                    .filter(|r| !left_subset.contains(r))
                    .cloned()
                    .collect();

                if right_subset.is_empty() {
                    continue;
                }

                // Check if we can join these subsets
                if !self.can_join_subsets(graph, &left_subset, &right_subset) {
                    continue;
                }

                // Get cached plans for left and right
                let (left_plan, _) = self.memo.get(&left_subset).unwrap();
                let (right_plan, _) = self.memo.get(&right_subset).unwrap();

                // Try all join algorithms
                for algo in [
                    JoinAlgorithm::NestedLoop,
                    JoinAlgorithm::MergeJoin,
                    JoinAlgorithm::HashJoin,
                ] {
                    let cost = self
                        .optimizer
                        .estimate_join_cost(left_plan, right_plan, algo);

                    if cost.total_cost < best_cost.total_cost {
                        let condition =
                            self.get_join_condition_str(graph, &left_subset, &right_subset);
                        let plan = PlanNode::Join {
                            left: Box::new(left_plan.clone()),
                            right: Box::new(right_plan.clone()),
                            algorithm: algo,
                            condition,
                            cost: cost.clone(),
                        };
                        best_plan = Some(plan);
                        best_cost = cost;
                    }
                }
            }
        }

        if let Some(plan) = best_plan {
            (plan, best_cost)
        } else {
            // No valid join found - create a cross product with hash join
            let (left_plan, _) = self.memo.get(&subset[..1].to_vec()).unwrap();
            let (right_plan, _) = self.memo.get(&subset[1..].to_vec()).unwrap();
            let cost =
                self.optimizer
                    .estimate_join_cost(left_plan, right_plan, JoinAlgorithm::HashJoin);
            let plan = PlanNode::Join {
                left: Box::new(left_plan.clone()),
                right: Box::new(right_plan.clone()),
                algorithm: JoinAlgorithm::HashJoin,
                condition: "CROSS JOIN".to_string(),
                cost: cost.clone(),
            };
            (plan, cost)
        }
    }

    /// Check if two subsets can be joined
    fn can_join_subsets(&self, graph: &JoinGraph, left: &[String], right: &[String]) -> bool {
        for l in left {
            for r in right {
                if graph.can_join(l, r) {
                    return true;
                }
            }
        }
        false
    }

    /// Get join condition string between subsets
    fn get_join_condition_str(
        &self,
        graph: &JoinGraph,
        left: &[String],
        right: &[String],
    ) -> String {
        for l in left {
            for r in right {
                if let Some(cond) = graph.get_join_condition(l, r) {
                    return cond.condition.clone();
                }
            }
        }
        "unknown".to_string()
    }

    /// Generate all subsets of a given size
    fn generate_subsets<T: Clone>(items: &[T], size: usize) -> Vec<Vec<T>> {
        if size == 0 {
            return vec![vec![]];
        }
        if size > items.len() {
            return vec![];
        }
        if size == items.len() {
            return vec![items.to_vec()];
        }

        let mut result = Vec::new();
        Self::generate_subsets_helper(items, size, 0, &mut vec![], &mut result);
        result
    }

    fn generate_subsets_helper<T: Clone>(
        items: &[T],
        size: usize,
        start: usize,
        current: &mut Vec<T>,
        result: &mut Vec<Vec<T>>,
    ) {
        if current.len() == size {
            result.push(current.clone());
            return;
        }

        for i in start..items.len() {
            current.push(items[i].clone());
            Self::generate_subsets_helper(items, size, i + 1, current, result);
            current.pop();
        }
    }

    /// Create a scan plan for a single table
    fn create_scan_plan(&self, table: &str) -> PlanNode {
        use crate::engine::advanced_optimizer::ScanMethod;

        let cost = self.optimizer.estimate_sequential_scan_cost(table, &None);
        PlanNode::Scan {
            table: table.to_string(),
            method: ScanMethod::Sequential,
            filter: None,
            cost,
        }
    }

    /// Optimize with Genetic Query Optimizer (GEQO) for large join graphs
    ///
    /// PostgreSQL uses this when the number of relations exceeds geqo_threshold.
    /// This is a simplified version - real GEQO is much more sophisticated.
    fn optimize_with_geqo(&self, graph: &JoinGraph) -> PlanNode {
        // Simplified: just use left-deep join tree with random order
        // Real GEQO uses genetic algorithm with crossover, mutation, etc.

        let relations = graph.relations.clone();
        let mut plan = self.create_scan_plan(&relations[0].name);

        for i in 1..relations.len() {
            let right = self.create_scan_plan(&relations[i].name);

            // Pick cheapest join algorithm
            let mut best_algo = JoinAlgorithm::HashJoin;
            let mut best_cost =
                self.optimizer
                    .estimate_join_cost(&plan, &right, JoinAlgorithm::HashJoin);

            for algo in [JoinAlgorithm::NestedLoop, JoinAlgorithm::MergeJoin] {
                let cost = self.optimizer.estimate_join_cost(&plan, &right, algo);
                if cost.total_cost < best_cost.total_cost {
                    best_algo = algo;
                    best_cost = cost;
                }
            }

            let condition = format!("join_{}", i);
            plan = PlanNode::Join {
                left: Box::new(plan),
                right: Box::new(right),
                algorithm: best_algo,
                condition,
                cost: best_cost,
            };
        }

        plan
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_subsets() {
        let items = vec!["A", "B", "C"];

        let subsets_1 = JoinOrderOptimizer::generate_subsets(&items, 1);
        assert_eq!(subsets_1.len(), 3);

        let subsets_2 = JoinOrderOptimizer::generate_subsets(&items, 2);
        assert_eq!(subsets_2.len(), 3); // AB, AC, BC

        let subsets_3 = JoinOrderOptimizer::generate_subsets(&items, 3);
        assert_eq!(subsets_3.len(), 1); // ABC
    }

    #[test]
    fn test_join_graph() {
        let relations = vec![
            Relation {
                name: "orders".to_string(),
                alias: None,
            },
            Relation {
                name: "customers".to_string(),
                alias: None,
            },
        ];

        let conditions = vec![JoinCondition {
            left_table: "orders".to_string(),
            right_table: "customers".to_string(),
            left_column: "customer_id".to_string(),
            right_column: "id".to_string(),
            condition: "orders.customer_id = customers.id".to_string(),
        }];

        let graph = JoinGraph::new(relations, conditions);
        assert!(graph.can_join("orders", "customers"));
        assert!(graph.can_join("customers", "orders"));
        assert!(!graph.can_join("orders", "products"));
    }

    #[test]
    fn test_two_table_join() {
        let optimizer = AdvancedOptimizer::new();
        let mut join_opt = JoinOrderOptimizer::new(optimizer);

        let relations = vec![
            Relation {
                name: "orders".to_string(),
                alias: None,
            },
            Relation {
                name: "customers".to_string(),
                alias: None,
            },
        ];

        let conditions = vec![JoinCondition {
            left_table: "orders".to_string(),
            right_table: "customers".to_string(),
            left_column: "customer_id".to_string(),
            right_column: "id".to_string(),
            condition: "orders.customer_id = customers.id".to_string(),
        }];

        let graph = JoinGraph::new(relations, conditions);
        let plan = join_opt.optimize_join_order(&graph);

        // Should produce a join plan
        match plan {
            PlanNode::Join { algorithm, .. } => {
                // Should pick hash join or another efficient algorithm
                assert!(matches!(
                    algorithm,
                    JoinAlgorithm::HashJoin | JoinAlgorithm::NestedLoop | JoinAlgorithm::MergeJoin
                ));
            }
            _ => panic!("Expected join plan"),
        }
    }

    #[test]
    fn test_three_table_join() {
        let optimizer = AdvancedOptimizer::new();
        let mut join_opt = JoinOrderOptimizer::new(optimizer);

        let relations = vec![
            Relation {
                name: "orders".to_string(),
                alias: None,
            },
            Relation {
                name: "customers".to_string(),
                alias: None,
            },
            Relation {
                name: "products".to_string(),
                alias: None,
            },
        ];

        let conditions = vec![
            JoinCondition {
                left_table: "orders".to_string(),
                right_table: "customers".to_string(),
                left_column: "customer_id".to_string(),
                right_column: "id".to_string(),
                condition: "orders.customer_id = customers.id".to_string(),
            },
            JoinCondition {
                left_table: "orders".to_string(),
                right_table: "products".to_string(),
                left_column: "product_id".to_string(),
                right_column: "id".to_string(),
                condition: "orders.product_id = products.id".to_string(),
            },
        ];

        let graph = JoinGraph::new(relations, conditions);
        let plan = join_opt.optimize_join_order(&graph);

        // Should produce nested joins
        match plan {
            PlanNode::Join { .. } => {
                // Success - we got a join plan
            }
            _ => panic!("Expected join plan"),
        }
    }
}
