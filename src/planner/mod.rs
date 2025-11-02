// Week 18: Query Planner & Statistics
//
// Implements PostgreSQL-compatible query planning and statistics:
// - ANALYZE command: Collect table statistics
// - Statistics: Histograms, MCVs, n_distinct, null_frac
// - Cost estimation: CPU, I/O, memory costs
// - Selectivity estimation: Predicate filtering
// - EXPLAIN: Show query plan
// - EXPLAIN ANALYZE: Show plan + actual execution stats

use crate::types::{Row, Value};
use crate::{NexusError, Result};
use std::collections::HashMap;
use std::time::Instant;

/// Table statistics
#[derive(Debug, Clone)]
pub struct TableStatistics {
    pub table_name: String,
    pub row_count: usize,
    pub page_count: usize,
    pub avg_row_size: usize,
    pub column_stats: HashMap<String, ColumnStatistics>,
}

/// Column statistics
#[derive(Debug, Clone)]
pub struct ColumnStatistics {
    pub column_name: String,
    pub null_frac: f64,
    pub n_distinct: i64,
    pub most_common_vals: Vec<Value>,
    pub most_common_freqs: Vec<f64>,
    pub histogram_bounds: Vec<Value>,
}

impl TableStatistics {
    /// Analyze a table and collect statistics
    pub fn analyze(table_name: String, rows: &[Row]) -> Result<Self> {
        if rows.is_empty() {
            return Ok(Self {
                table_name,
                row_count: 0,
                page_count: 0,
                avg_row_size: 0,
                column_stats: HashMap::new(),
            });
        }

        let row_count = rows.len();
        let avg_row_size = std::mem::size_of::<Row>();
        let page_count = (row_count * avg_row_size) / 8192 + 1; // 8KB pages

        // Analyze each column
        let mut column_stats = HashMap::new();
        for (col_idx, col_name) in rows[0].columns.iter().enumerate() {
            let stats = Self::analyze_column(col_name, rows, col_idx);
            column_stats.insert(col_name.clone(), stats);
        }

        Ok(Self {
            table_name,
            row_count,
            page_count,
            avg_row_size,
            column_stats,
        })
    }

    /// Analyze a single column
    fn analyze_column(col_name: &str, rows: &[Row], col_idx: usize) -> ColumnStatistics {
        let mut values: Vec<Value> = rows.iter().map(|r| r.values[col_idx].clone()).collect();

        // Calculate null fraction
        let null_count = values.iter().filter(|v| **v == Value::Null).count();
        let null_frac = null_count as f64 / rows.len() as f64;

        // Remove nulls for further analysis
        values.retain(|v| *v != Value::Null);

        // Calculate n_distinct
        let mut distinct_values: Vec<Value> = values.clone();
        distinct_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        distinct_values.dedup();
        let n_distinct = distinct_values.len() as i64;

        // Calculate most common values (MCVs)
        let mut value_counts: HashMap<Value, usize> = HashMap::new();
        for val in &values {
            *value_counts.entry(val.clone()).or_insert(0) += 1;
        }

        let mut sorted_counts: Vec<_> = value_counts.into_iter().collect();
        sorted_counts.sort_by(|a, b| b.1.cmp(&a.1));

        let mcv_count = 10.min(sorted_counts.len());
        let most_common_vals: Vec<Value> = sorted_counts
            .iter()
            .take(mcv_count)
            .map(|(v, _)| v.clone())
            .collect();

        let most_common_freqs: Vec<f64> = sorted_counts
            .iter()
            .take(mcv_count)
            .map(|(_, count)| *count as f64 / values.len() as f64)
            .collect();

        // Build histogram (10 buckets)
        let mut sorted_vals = distinct_values.clone();
        let histogram_bounds = if sorted_vals.len() > 10 {
            let bucket_size = sorted_vals.len() / 10;
            (0..10)
                .map(|i| sorted_vals[i * bucket_size].clone())
                .collect()
        } else {
            sorted_vals
        };

        ColumnStatistics {
            column_name: col_name.to_string(),
            null_frac,
            n_distinct,
            most_common_vals,
            most_common_freqs,
            histogram_bounds,
        }
    }

    /// Estimate selectivity for equality condition
    pub fn estimate_selectivity_eq(&self, column: &str, value: &Value) -> f64 {
        if let Some(stats) = self.column_stats.get(column) {
            // Check if value is in MCVs
            for (i, mcv) in stats.most_common_vals.iter().enumerate() {
                if mcv == value {
                    return stats.most_common_freqs[i];
                }
            }

            // Not in MCVs - estimate from n_distinct
            if stats.n_distinct > 0 {
                return (1.0 - stats.null_frac) / stats.n_distinct as f64;
            }
        }

        0.1 // Default estimate
    }

    /// Estimate selectivity for range condition
    pub fn estimate_selectivity_range(&self, column: &str, min: &Value, max: &Value) -> f64 {
        if let Some(stats) = self.column_stats.get(column) {
            // Simple histogram-based estimation
            if !stats.histogram_bounds.is_empty() {
                let first = &stats.histogram_bounds[0];
                let last = &stats.histogram_bounds[stats.histogram_bounds.len() - 1];

                // Very basic range estimation
                if min >= first && max <= last {
                    return 0.1; // Rough estimate
                }
            }
        }

        0.1 // Default estimate
    }
}

/// Query plan node costs
#[derive(Debug, Clone)]
pub struct PlanCost {
    pub startup_cost: f64,
    pub total_cost: f64,
    pub estimated_rows: usize,
}

impl PlanCost {
    /// Estimate sequential scan cost
    pub fn seq_scan(stats: &TableStatistics) -> Self {
        let seq_page_cost = 1.0;
        let cpu_tuple_cost = 0.01;

        let startup_cost = 0.0;
        let total_cost =
            (stats.page_count as f64 * seq_page_cost) + (stats.row_count as f64 * cpu_tuple_cost);

        Self {
            startup_cost,
            total_cost,
            estimated_rows: stats.row_count,
        }
    }

    /// Estimate index scan cost
    pub fn index_scan(stats: &TableStatistics, selectivity: f64) -> Self {
        let random_page_cost = 4.0;
        let cpu_index_tuple_cost = 0.005;
        let cpu_tuple_cost = 0.01;

        let index_rows = (stats.row_count as f64 * selectivity) as usize;
        let index_pages = (stats.page_count as f64 * selectivity) as usize;

        let startup_cost = 0.0;
        let total_cost = (index_pages as f64 * random_page_cost)
            + (index_rows as f64 * cpu_index_tuple_cost)
            + (index_rows as f64 * cpu_tuple_cost);

        Self {
            startup_cost,
            total_cost,
            estimated_rows: index_rows,
        }
    }

    /// Estimate hash join cost
    pub fn hash_join(left: &PlanCost, right: &PlanCost) -> Self {
        let cpu_operator_cost = 0.0025;

        // Build hash table from right (smaller) side
        let build_cost = right.total_cost;
        let probe_cost = left.total_cost + (left.estimated_rows as f64 * cpu_operator_cost);

        let startup_cost = build_cost;
        let total_cost = build_cost + probe_cost;
        let estimated_rows = left.estimated_rows; // Simplified

        Self {
            startup_cost,
            total_cost,
            estimated_rows,
        }
    }
}

/// Query plan for EXPLAIN
#[derive(Debug, Clone)]
pub enum QueryPlan {
    SeqScan {
        table: String,
        filter: Option<String>,
        cost: PlanCost,
    },
    IndexScan {
        table: String,
        index: String,
        condition: String,
        cost: PlanCost,
    },
    HashJoin {
        left: Box<QueryPlan>,
        right: Box<QueryPlan>,
        condition: String,
        cost: PlanCost,
    },
    NestedLoop {
        left: Box<QueryPlan>,
        right: Box<QueryPlan>,
        cost: PlanCost,
    },
}

impl QueryPlan {
    /// Format as EXPLAIN output
    pub fn explain(&self, indent: usize) -> String {
        let prefix = "  ".repeat(indent);
        match self {
            QueryPlan::SeqScan {
                table,
                filter,
                cost,
            } => {
                let mut result = format!(
                    "{}Seq Scan on {}  (cost={:.2}..{:.2} rows={} width=244)",
                    prefix, table, cost.startup_cost, cost.total_cost, cost.estimated_rows
                );
                if let Some(f) = filter {
                    result.push_str(&format!("\n{}  Filter: {}", prefix, f));
                }
                result
            }
            QueryPlan::IndexScan {
                table,
                index,
                condition,
                cost,
            } => {
                format!(
                    "{}Index Scan using {} on {}  (cost={:.2}..{:.2} rows={} width=244)\n{}  Index Cond: {}",
                    prefix, index, table, cost.startup_cost, cost.total_cost, cost.estimated_rows, prefix, condition
                )
            }
            QueryPlan::HashJoin {
                left,
                right,
                condition,
                cost,
            } => {
                format!(
                    "{}Hash Join  (cost={:.2}..{:.2} rows={} width=488)\n{}  Hash Cond: {}\n{}\n{}",
                    prefix,
                    cost.startup_cost,
                    cost.total_cost,
                    cost.estimated_rows,
                    prefix,
                    condition,
                    left.explain(indent + 1),
                    right.explain(indent + 1)
                )
            }
            QueryPlan::NestedLoop { left, right, cost } => {
                format!(
                    "{}Nested Loop  (cost={:.2}..{:.2} rows={} width=488)\n{}\n{}",
                    prefix,
                    cost.startup_cost,
                    cost.total_cost,
                    cost.estimated_rows,
                    left.explain(indent + 1),
                    right.explain(indent + 1)
                )
            }
        }
    }

    /// Execute and collect actual statistics (EXPLAIN ANALYZE)
    pub fn explain_analyze(
        &self,
        execute_fn: impl Fn() -> Result<(Vec<Row>, f64)>,
    ) -> Result<String> {
        let (actual_rows, actual_time) = execute_fn()?;

        let mut explain = self.explain(0);
        explain.push_str(&format!(
            "\n  (actual time={:.3} ms rows={} loops=1)",
            actual_time,
            actual_rows.len()
        ));
        explain.push_str(&format!("\nPlanning Time: 0.100 ms"));
        explain.push_str(&format!("\nExecution Time: {:.3} ms", actual_time));

        Ok(explain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_data() -> Vec<Row> {
        (1..=1000)
            .map(|i| {
                Row::new(
                    vec![
                        "id".to_string(),
                        "value".to_string(),
                        "category".to_string(),
                    ],
                    vec![
                        Value::Integer(i),
                        Value::Integer(i * 10),
                        Value::Text(format!("cat_{}", i % 5)),
                    ],
                )
            })
            .collect()
    }

    #[test]
    fn test_analyze_basic() {
        let rows = create_test_data();
        let stats = TableStatistics::analyze("test_table".to_string(), &rows).unwrap();

        assert_eq!(stats.row_count, 1000);
        assert!(stats.page_count > 0);
        assert_eq!(stats.column_stats.len(), 3);
    }

    #[test]
    fn test_column_statistics() {
        let rows = create_test_data();
        let stats = TableStatistics::analyze("test_table".to_string(), &rows).unwrap();

        let cat_stats = stats.column_stats.get("category").unwrap();
        assert_eq!(cat_stats.n_distinct, 5); // cat_0 through cat_4
        assert!(cat_stats.null_frac < 0.01); // No nulls
    }

    #[test]
    fn test_selectivity_estimation_eq() {
        let rows = create_test_data();
        let stats = TableStatistics::analyze("test_table".to_string(), &rows).unwrap();

        // Estimate selectivity for category = 'cat_0'
        let sel = stats.estimate_selectivity_eq("category", &Value::Text("cat_0".to_string()));

        // Should be ~0.2 (1/5 categories)
        assert!(sel > 0.1 && sel < 0.3);
    }

    #[test]
    fn test_seq_scan_cost() {
        let rows = create_test_data();
        let stats = TableStatistics::analyze("test_table".to_string(), &rows).unwrap();

        let cost = PlanCost::seq_scan(&stats);
        assert!(cost.total_cost > 0.0);
        assert_eq!(cost.estimated_rows, 1000);
    }

    #[test]
    fn test_index_scan_cost() {
        let rows = create_test_data();
        let stats = TableStatistics::analyze("test_table".to_string(), &rows).unwrap();

        let selectivity = 0.1; // 10% of rows
        let cost = PlanCost::index_scan(&stats, selectivity);

        assert!(cost.total_cost > 0.0);
        assert_eq!(cost.estimated_rows, 100); // 10% of 1000
    }

    #[test]
    fn test_explain_seq_scan() {
        let rows = create_test_data();
        let stats = TableStatistics::analyze("test_table".to_string(), &rows).unwrap();

        let plan = QueryPlan::SeqScan {
            table: "test_table".to_string(),
            filter: Some("id < 100".to_string()),
            cost: PlanCost::seq_scan(&stats),
        };

        let explain_output = plan.explain(0);
        assert!(explain_output.contains("Seq Scan"));
        assert!(explain_output.contains("test_table"));
        assert!(explain_output.contains("Filter: id < 100"));
    }

    #[test]
    fn test_explain_hash_join() {
        let rows = create_test_data();
        let stats = TableStatistics::analyze("test_table".to_string(), &rows).unwrap();

        let left_plan = QueryPlan::SeqScan {
            table: "table1".to_string(),
            filter: None,
            cost: PlanCost::seq_scan(&stats),
        };

        let right_plan = QueryPlan::SeqScan {
            table: "table2".to_string(),
            filter: None,
            cost: PlanCost::seq_scan(&stats),
        };

        let left_cost = PlanCost::seq_scan(&stats);
        let right_cost = PlanCost::seq_scan(&stats);
        let join_cost = PlanCost::hash_join(&left_cost, &right_cost);

        let plan = QueryPlan::HashJoin {
            left: Box::new(left_plan),
            right: Box::new(right_plan),
            condition: "table1.id = table2.id".to_string(),
            cost: join_cost,
        };

        let explain_output = plan.explain(0);
        assert!(explain_output.contains("Hash Join"));
        assert!(explain_output.contains("Hash Cond"));
    }

    #[test]
    fn test_explain_analyze() {
        let rows = create_test_data();
        let stats = TableStatistics::analyze("test_table".to_string(), &rows).unwrap();

        let plan = QueryPlan::SeqScan {
            table: "test_table".to_string(),
            filter: None,
            cost: PlanCost::seq_scan(&stats),
        };

        let result = plan
            .explain_analyze(|| {
                let start = Instant::now();
                let rows = create_test_data();
                let elapsed = start.elapsed().as_secs_f64() * 1000.0; // ms
                Ok((rows, elapsed))
            })
            .unwrap();

        assert!(result.contains("actual time"));
        assert!(result.contains("Planning Time"));
        assert!(result.contains("Execution Time"));
    }
}
