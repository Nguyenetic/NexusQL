//! Batch Operations - PostgreSQL/DuckDB Best Practice
//! PROPRIETARY - Performance Optimization

use crate::types::Value;

/// Batch insert builder
#[allow(dead_code)]
pub struct BatchInsert {
    table: String,
    columns: Vec<String>,
    rows: Vec<Vec<Value>>,
}

impl BatchInsert {
    pub fn new(table: String, columns: Vec<String>) -> Self {
        Self {
            table,
            columns,
            rows: Vec::new(),
        }
    }

    /// Add a row to the batch
    pub fn add_row(&mut self, values: Vec<Value>) {
        self.rows.push(values);
    }

    /// Generate optimized SQL
    pub fn to_sql(&self) -> String {
        if self.rows.is_empty() {
            return String::new();
        }

        let values_clauses: Vec<String> = self
            .rows
            .iter()
            .map(|row| {
                let values: Vec<String> = row.iter().map(|v| Self::value_to_sql(v)).collect();
                format!("({})", values.join(", "))
            })
            .collect();

        format!(
            "INSERT INTO {} VALUES {}",
            self.table,
            values_clauses.join(", ")
        )
    }

    fn value_to_sql(value: &Value) -> String {
        match value {
            Value::Integer(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Text(s) => format!("'{}'", s.replace('\'', "''")),
            Value::Geometry(g) => g.as_sql(),
            Value::Boolean(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
            Value::Null => "NULL".to_string(),
            Value::Blob(_) => "NULL".to_string(),
            Value::Vector(v) => format!(
                "'[{}]'",
                v.iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }

    /// Get number of rows in batch
    pub fn len(&self) -> usize {
        self.rows.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_insert() {
        let mut batch = BatchInsert::new("users".to_string(), vec!["id".into(), "name".into()]);

        batch.add_row(vec![Value::Integer(1), Value::Text("Alice".into())]);
        batch.add_row(vec![Value::Integer(2), Value::Text("Bob".into())]);

        let sql = batch.to_sql();

        assert!(sql.contains("INSERT INTO users"));
        assert!(sql.contains("(1, 'Alice')"));
        assert!(sql.contains("(2, 'Bob')"));
    }
}
