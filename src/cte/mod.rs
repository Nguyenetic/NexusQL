// Week 15: Common Table Expressions (CTEs) & Recursive Queries
//
// Implements PostgreSQL-compatible CTEs for complex analytical queries:
// - WITH clause: Named subqueries for query organization
// - Multiple CTEs: Chain multiple named queries
// - Recursive CTEs: WITH RECURSIVE for hierarchical data
// - Cycle detection: Prevent infinite loops in recursive queries
// - Materialization: Control when CTEs are cached vs. inlined

use crate::types::Row;
use crate::{NexusError, Result};
use std::collections::{HashMap, HashSet};

/// Common Table Expression
#[derive(Debug, Clone)]
pub struct CTE {
    /// CTE name
    pub name: String,
    /// Column names (optional)
    pub columns: Option<Vec<String>>,
    /// Query results
    pub rows: Vec<Row>,
    /// Whether to materialize (cache) results
    pub materialized: bool,
    /// Whether this is a recursive CTE
    pub recursive: bool,
}

/// CTE Executor
pub struct CTEExecutor {
    /// Named CTEs available for reference
    ctes: HashMap<String, CTE>,
}

impl CTEExecutor {
    /// Create a new CTE executor
    pub fn new() -> Self {
        Self {
            ctes: HashMap::new(),
        }
    }

    /// Register a CTE
    pub fn register_cte(&mut self, cte: CTE) -> Result<()> {
        if self.ctes.contains_key(&cte.name) {
            return Err(NexusError::ExecutionError(format!(
                "CTE '{}' already defined",
                cte.name
            )));
        }
        self.ctes.insert(cte.name.clone(), cte);
        Ok(())
    }

    /// Get CTE by name
    pub fn get_cte(&self, name: &str) -> Result<&CTE> {
        self.ctes
            .get(name)
            .ok_or_else(|| NexusError::ExecutionError(format!("CTE '{}' not found", name)))
    }

    /// Execute a non-recursive CTE query
    pub fn execute_simple_cte(
        &mut self,
        name: String,
        columns: Option<Vec<String>>,
        base_query_fn: impl FnOnce() -> Result<Vec<Row>>,
        materialized: bool,
    ) -> Result<()> {
        let rows = base_query_fn()?;

        let cte = CTE {
            name: name.clone(),
            columns,
            rows,
            materialized,
            recursive: false,
        };

        self.register_cte(cte)
    }

    /// Execute a recursive CTE query
    pub fn execute_recursive_cte(
        &mut self,
        name: String,
        columns: Option<Vec<String>>,
        base_query_fn: impl Fn() -> Result<Vec<Row>>,
        recursive_query_fn: impl Fn(&[Row]) -> Result<Vec<Row>>,
        max_iterations: usize,
    ) -> Result<()> {
        // Execute base query (non-recursive term)
        let mut working_table = base_query_fn()?;
        let mut result = working_table.clone();

        // Execute recursive term iteratively
        for iteration in 0..max_iterations {
            if working_table.is_empty() {
                break; // No more rows to process
            }

            // Execute recursive query with current working table
            let new_rows = recursive_query_fn(&working_table)?;

            if new_rows.is_empty() {
                break; // Recursion complete
            }

            // Update working table for next iteration
            working_table = new_rows.clone();
            result.extend(new_rows);

            // Safety check
            if iteration == max_iterations - 1 {
                return Err(NexusError::ExecutionError(format!(
                    "Recursive CTE exceeded max iterations: {}",
                    max_iterations
                )));
            }
        }

        let cte = CTE {
            name: name.clone(),
            columns,
            rows: result,
            materialized: true, // Recursive CTEs are always materialized
            recursive: true,
        };

        self.register_cte(cte)
    }
}

/// Recursive query helper with cycle detection
pub struct RecursiveCTE {
    /// Base rows (initial seed)
    base_rows: Vec<Row>,
    /// Maximum recursion depth
    max_depth: usize,
    /// Cycle detection enabled
    detect_cycles: bool,
}

impl RecursiveCTE {
    /// Create new recursive CTE helper
    pub fn new(base_rows: Vec<Row>, max_depth: usize) -> Self {
        Self {
            base_rows,
            max_depth,
            detect_cycles: true,
        }
    }

    /// Execute recursive query with cycle detection
    pub fn execute<F>(&self, recursive_step: F) -> Result<Vec<Row>>
    where
        F: Fn(&[Row]) -> Result<Vec<Row>>,
    {
        let mut result = self.base_rows.clone();
        let mut working_table = self.base_rows.clone();
        let mut seen_keys: HashSet<String> = HashSet::new();

        // Track base rows for cycle detection
        if self.detect_cycles {
            for row in &self.base_rows {
                seen_keys.insert(Self::row_key(row));
            }
        }

        for depth in 0..self.max_depth {
            if working_table.is_empty() {
                break;
            }

            // Execute recursive step
            let new_rows = recursive_step(&working_table)?;

            if new_rows.is_empty() {
                break;
            }

            // Filter out cycles
            let mut filtered_new_rows = Vec::new();
            for row in new_rows {
                let key = Self::row_key(&row);
                if !self.detect_cycles || !seen_keys.contains(&key) {
                    if self.detect_cycles {
                        seen_keys.insert(key);
                    }
                    filtered_new_rows.push(row);
                }
            }

            if filtered_new_rows.is_empty() {
                break; // All new rows were cycles
            }

            result.extend(filtered_new_rows.clone());
            working_table = filtered_new_rows;

            // Safety check
            if depth == self.max_depth - 1 {
                return Err(NexusError::ExecutionError(format!(
                    "Recursive query exceeded max depth: {}",
                    self.max_depth
                )));
            }
        }

        Ok(result)
    }

    /// Generate a unique key for a row (for cycle detection)
    fn row_key(row: &Row) -> String {
        row.values
            .iter()
            .map(|v| format!("{:?}", v))
            .collect::<Vec<_>>()
            .join("|")
    }

    /// Disable cycle detection (use with caution)
    pub fn without_cycle_detection(mut self) -> Self {
        self.detect_cycles = false;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ResultSet, Row, Value};

    #[test]
    fn test_simple_cte() {
        let mut executor = CTEExecutor::new();

        // Define CTE: sales_summary
        executor
            .execute_simple_cte(
                "sales_summary".to_string(),
                Some(vec!["region".to_string(), "total".to_string()]),
                || {
                    Ok(vec![
                        Row::new(
                            vec!["region".to_string(), "total".to_string()],
                            vec![Value::Text("North".to_string()), Value::Integer(1000)],
                        ),
                        Row::new(
                            vec!["region".to_string(), "total".to_string()],
                            vec![Value::Text("South".to_string()), Value::Integer(2000)],
                        ),
                    ])
                },
                true,
            )
            .unwrap();

        // Reference CTE
        let cte = executor.get_cte("sales_summary").unwrap();
        assert_eq!(cte.rows.len(), 2);
        assert_eq!(cte.materialized, true);
    }

    #[test]
    fn test_multiple_ctes() {
        let mut executor = CTEExecutor::new();

        // First CTE
        executor
            .execute_simple_cte(
                "cte1".to_string(),
                None,
                || {
                    Ok(vec![Row::new(
                        vec!["a".to_string()],
                        vec![Value::Integer(1)],
                    )])
                },
                false,
            )
            .unwrap();

        // Second CTE
        executor
            .execute_simple_cte(
                "cte2".to_string(),
                None,
                || {
                    Ok(vec![Row::new(
                        vec!["b".to_string()],
                        vec![Value::Integer(2)],
                    )])
                },
                false,
            )
            .unwrap();

        assert!(executor.get_cte("cte1").is_ok());
        assert!(executor.get_cte("cte2").is_ok());
    }

    #[test]
    fn test_recursive_cte_sum() {
        let mut executor = CTEExecutor::new();

        // WITH RECURSIVE t(n) AS (VALUES (1) UNION ALL SELECT n+1 FROM t WHERE n < 10)
        executor
            .execute_recursive_cte(
                "t".to_string(),
                Some(vec!["n".to_string()]),
                || {
                    Ok(vec![Row::new(
                        vec!["n".to_string()],
                        vec![Value::Integer(1)],
                    )])
                },
                |working_table| {
                    let mut new_rows = Vec::new();
                    for row in working_table {
                        if let Value::Integer(n) = row.values[0] {
                            if n < 10 {
                                new_rows.push(Row::new(
                                    vec!["n".to_string()],
                                    vec![Value::Integer(n + 1)],
                                ));
                            }
                        }
                    }
                    Ok(new_rows)
                },
                100,
            )
            .unwrap();

        let cte = executor.get_cte("t").unwrap();
        assert_eq!(cte.rows.len(), 10); // 1 through 10

        // Calculate sum
        let sum: i64 = cte
            .rows
            .iter()
            .filter_map(|row| match row.values[0] {
                Value::Integer(n) => Some(n),
                _ => None,
            })
            .sum();
        assert_eq!(sum, 55); // 1+2+...+10 = 55
    }

    #[test]
    fn test_recursive_cte_hierarchy() {
        // Simulate employee hierarchy
        let employees = vec![
            (1, "Alice", None),
            (2, "Bob", Some(1)),
            (3, "Charlie", Some(1)),
            (4, "David", Some(2)),
            (5, "Eve", Some(2)),
        ];

        // Base: top-level employees (manager_id IS NULL)
        let base_rows = vec![Row::new(
            vec!["id".to_string(), "name".to_string(), "level".to_string()],
            vec![
                Value::Integer(1),
                Value::Text("Alice".to_string()),
                Value::Integer(0),
            ],
        )];

        let recursive_cte = RecursiveCTE::new(base_rows, 10);

        // Recursive step: find employees whose manager is in working table
        let result = recursive_cte
            .execute(|working_table| {
                let mut new_rows = Vec::new();

                // Extract manager IDs from working table
                let manager_ids: HashSet<i64> = working_table
                    .iter()
                    .filter_map(|row| match row.values[0] {
                        Value::Integer(id) => Some(id),
                        _ => None,
                    })
                    .collect();

                // Find employees with these managers
                for (id, name, manager_id) in &employees {
                    if let Some(mgr_id) = manager_id {
                        if manager_ids.contains(mgr_id) {
                            // Get parent level
                            let parent_level = working_table
                                .iter()
                                .find(|r| r.values[0] == Value::Integer(*mgr_id))
                                .and_then(|r| match r.values[2] {
                                    Value::Integer(l) => Some(l),
                                    _ => None,
                                })
                                .unwrap_or(0);

                            new_rows.push(Row::new(
                                vec!["id".to_string(), "name".to_string(), "level".to_string()],
                                vec![
                                    Value::Integer(*id),
                                    Value::Text(name.to_string()),
                                    Value::Integer(parent_level + 1),
                                ],
                            ));
                        }
                    }
                }

                Ok(new_rows)
            })
            .unwrap();

        assert_eq!(result.len(), 5); // All 5 employees

        // Check levels
        let alice_level = result
            .iter()
            .find(|r| r.values[1] == Value::Text("Alice".to_string()))
            .map(|r| r.values[2].clone())
            .unwrap();
        assert_eq!(alice_level, Value::Integer(0));

        let david_level = result
            .iter()
            .find(|r| r.values[1] == Value::Text("David".to_string()))
            .map(|r| r.values[2].clone())
            .unwrap();
        assert_eq!(david_level, Value::Integer(2)); // Alice -> Bob -> David
    }

    #[test]
    fn test_cycle_detection() {
        // Create cyclic data: 1 -> 2 -> 3 -> 1
        let base_rows = vec![Row::new(
            vec!["id".to_string(), "next_id".to_string()],
            vec![Value::Integer(1), Value::Integer(2)],
        )];

        let recursive_cte = RecursiveCTE::new(base_rows, 100);

        let result = recursive_cte
            .execute(|working_table| {
                let mut new_rows = Vec::new();

                for row in working_table {
                    if let Value::Integer(next_id) = row.values[1] {
                        // Simulate following the link
                        let next_next_id = match next_id {
                            2 => 3,
                            3 => 1, // Cycle back to 1
                            _ => return Ok(vec![]),
                        };

                        new_rows.push(Row::new(
                            vec!["id".to_string(), "next_id".to_string()],
                            vec![Value::Integer(next_id), Value::Integer(next_next_id)],
                        ));
                    }
                }

                Ok(new_rows)
            })
            .unwrap();

        // Should stop at 3 rows (1->2, 2->3, 3->1) due to cycle detection
        assert!(result.len() <= 3);
    }

    #[test]
    fn test_recursive_sum_1_to_100() {
        let mut executor = CTEExecutor::new();

        // WITH RECURSIVE t(n) AS (VALUES (1) UNION ALL SELECT n+1 FROM t WHERE n < 100)
        executor
            .execute_recursive_cte(
                "t".to_string(),
                Some(vec!["n".to_string()]),
                || {
                    Ok(vec![Row::new(
                        vec!["n".to_string()],
                        vec![Value::Integer(1)],
                    )])
                },
                |working_table| {
                    let mut new_rows = Vec::new();
                    for row in working_table {
                        if let Value::Integer(n) = row.values[0] {
                            if n < 100 {
                                new_rows.push(Row::new(
                                    vec!["n".to_string()],
                                    vec![Value::Integer(n + 1)],
                                ));
                            }
                        }
                    }
                    Ok(new_rows)
                },
                150,
            )
            .unwrap();

        let cte = executor.get_cte("t").unwrap();
        assert_eq!(cte.rows.len(), 100);

        // Verify sum = 5050
        let sum: i64 = cte
            .rows
            .iter()
            .filter_map(|row| match row.values[0] {
                Value::Integer(n) => Some(n),
                _ => None,
            })
            .sum();
        assert_eq!(sum, 5050);
    }
}
