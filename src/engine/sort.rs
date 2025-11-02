//! ORDER BY Implementation
//! PROPRIETARY

use crate::types::{Row, Value};

/// Sort direction
#[derive(Debug, Clone, PartialEq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Sort key (column + direction)
#[derive(Debug, Clone)]
pub struct SortKey {
    pub column: String,
    pub direction: SortDirection,
}

/// Sorting executor
pub struct SortExecutor;

impl SortExecutor {
    /// Sort rows by multiple keys
    pub fn sort_rows(mut rows: Vec<Row>, sort_keys: Vec<SortKey>) -> Vec<Row> {
        rows.sort_by(|a, b| {
            for key in &sort_keys {
                let a_val = a.get(&key.column);
                let b_val = b.get(&key.column);
                
                let ordering = match (a_val, b_val) {
                    (Some(av), Some(bv)) => Self::compare_values(av, bv),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                };
                
                let final_ordering = if key.direction == SortDirection::Descending {
                    ordering.reverse()
                } else {
                    ordering
                };
                
                if final_ordering != std::cmp::Ordering::Equal {
                    return final_ordering;
                }
            }
            
            std::cmp::Ordering::Equal
        });
        
        rows
    }
    
    fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
        match (a, b) {
            (Value::Integer(x), Value::Integer(y)) => x.cmp(y),
            (Value::Float(x), Value::Float(y)) => {
                x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
            },
            (Value::Text(x), Value::Text(y)) => x.cmp(y),
            (Value::Boolean(x), Value::Boolean(y)) => x.cmp(y),
            (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
            (Value::Null, _) => std::cmp::Ordering::Less,
            (_, Value::Null) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        }
    }
    
    /// Remove duplicate rows
    pub fn distinct(rows: Vec<Row>) -> Vec<Row> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        
        for row in rows {
            let key = Self::row_to_key(&row);
            if seen.insert(key) {
                result.push(row);
            }
        }
        
        result
    }
    
    fn row_to_key(row: &Row) -> String {
        row.values.iter()
            .map(|v| format!("{:?}", v))
            .collect::<Vec<_>>()
            .join("|")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sort_ascending() {
        let rows = vec![
            Row::new(vec!["age".into()], vec![Value::Integer(30)]),
            Row::new(vec!["age".into()], vec![Value::Integer(20)]),
            Row::new(vec!["age".into()], vec![Value::Integer(25)]),
        ];
        
        let sorted = SortExecutor::sort_rows(rows, vec![
            SortKey { column: "age".to_string(), direction: SortDirection::Ascending }
        ]);
        
        assert_eq!(sorted[0].get("age"), Some(&Value::Integer(20)));
        assert_eq!(sorted[1].get("age"), Some(&Value::Integer(25)));
        assert_eq!(sorted[2].get("age"), Some(&Value::Integer(30)));
    }
    
    #[test]
    fn test_sort_descending() {
        let rows = vec![
            Row::new(vec!["score".into()], vec![Value::Float(8.5)]),
            Row::new(vec!["score".into()], vec![Value::Float(9.2)]),
            Row::new(vec!["score".into()], vec![Value::Float(7.1)]),
        ];
        
        let sorted = SortExecutor::sort_rows(rows, vec![
            SortKey { column: "score".to_string(), direction: SortDirection::Descending }
        ]);
        
        if let Some(Value::Float(score)) = sorted[0].get("score") {
            assert!(score > &9.0);
        }
    }
    
    #[test]
    fn test_distinct() {
        let rows = vec![
            Row::new(vec!["name".into()], vec![Value::Text("Alice".into())]),
            Row::new(vec!["name".into()], vec![Value::Text("Bob".into())]),
            Row::new(vec!["name".into()], vec![Value::Text("Alice".into())]),
        ];
        
        let distinct = SortExecutor::distinct(rows);
        assert_eq!(distinct.len(), 2);  // Only 2 unique names
    }
}
