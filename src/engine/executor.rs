//! SQL Execution Engine
//! PROPRIETARY

use crate::query::ast::*;
use crate::query::evaluator::{evaluate_expr, evaluate_where, QueryParams};
use crate::storage::Table;
use crate::types::{ResultSet, Row, Value};
use crate::Result;
use ordered_float::OrderedFloat;
use std::collections::HashMap;

/// Check if a SELECT list contains aggregate functions
fn has_aggregate_functions(select_items: &[SelectItem]) -> bool {
    for item in select_items {
        if let SelectItem::Expr { expr, .. } = item {
            if is_aggregate_expr(expr) {
                return true;
            }
        }
    }
    false
}

/// Check if an expression is an aggregate function
fn is_aggregate_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Function { name, .. } => {
            let name_upper = name.to_uppercase();
            matches!(name_upper.as_str(), "COUNT" | "SUM" | "AVG" | "MIN" | "MAX")
        }
        _ => false,
    }
}

/// Evaluate an aggregate function over a set of rows
fn evaluate_aggregate(rows: &[Row], name: &str, args: &[Expr], params: &QueryParams) -> Value {
    let name_upper = name.to_uppercase();

    match name_upper.as_str() {
        "COUNT" => {
            // COUNT(*) counts all rows, COUNT(column) counts non-NULL values
            if args.is_empty() || matches!(args.first(), Some(Expr::Wildcard)) {
                Value::Integer(rows.len() as i64)
            } else {
                // Count non-NULL values
                let count = rows
                    .iter()
                    .filter(|row| {
                        let val = evaluate_expr(row, &args[0], params);
                        !matches!(val, Value::Null)
                    })
                    .count();
                Value::Integer(count as i64)
            }
        }
        "SUM" => {
            if args.is_empty() {
                return Value::Null;
            }
            let mut sum: f64 = 0.0;
            let mut has_value = false;
            for row in rows {
                let val = evaluate_expr(row, &args[0], params);
                match val {
                    Value::Integer(i) => {
                        sum += i as f64;
                        has_value = true;
                    }
                    Value::Float(f) => {
                        sum += f.0;
                        has_value = true;
                    }
                    _ => {}
                }
            }
            if has_value {
                Value::Float(OrderedFloat(sum))
            } else {
                Value::Null
            }
        }
        "AVG" => {
            if args.is_empty() {
                return Value::Null;
            }
            let mut sum: f64 = 0.0;
            let mut count: i64 = 0;
            for row in rows {
                let val = evaluate_expr(row, &args[0], params);
                match val {
                    Value::Integer(i) => {
                        sum += i as f64;
                        count += 1;
                    }
                    Value::Float(f) => {
                        sum += f.0;
                        count += 1;
                    }
                    _ => {}
                }
            }
            if count > 0 {
                Value::Float(OrderedFloat(sum / count as f64))
            } else {
                Value::Null
            }
        }
        "MIN" => {
            if args.is_empty() {
                return Value::Null;
            }
            let mut min_val: Option<Value> = None;
            for row in rows {
                let val = evaluate_expr(row, &args[0], params);
                if matches!(val, Value::Null) {
                    continue;
                }
                min_val = Some(match min_val {
                    None => val,
                    Some(current) => {
                        if val < current {
                            val
                        } else {
                            current
                        }
                    }
                });
            }
            min_val.unwrap_or(Value::Null)
        }
        "MAX" => {
            if args.is_empty() {
                return Value::Null;
            }
            let mut max_val: Option<Value> = None;
            for row in rows {
                let val = evaluate_expr(row, &args[0], params);
                if matches!(val, Value::Null) {
                    continue;
                }
                max_val = Some(match max_val {
                    None => val,
                    Some(current) => {
                        if val > current {
                            val
                        } else {
                            current
                        }
                    }
                });
            }
            max_val.unwrap_or(Value::Null)
        }
        _ => Value::Null,
    }
}

pub struct Executor {
    tables: HashMap<String, Table>,
    transaction_active: bool,
    transaction_snapshot: Option<HashMap<String, Table>>,
}

impl Executor {
    pub fn new() -> Self {
        Self {
            tables: HashMap::new(),
            transaction_active: false,
            transaction_snapshot: None,
        }
    }

    pub fn execute(&mut self, statement: &Statement, params: &QueryParams) -> Result<ResultSet> {
        match statement {
            Statement::CreateTable {
                name,
                columns,
                if_not_exists,
            } => {
                // If IF NOT EXISTS is specified and table already exists, do nothing
                if *if_not_exists && self.tables.contains_key(name) {
                    return Ok(ResultSet::empty());
                }

                let col_names: Vec<String> = columns.iter().map(|c| c.name.clone()).collect();
                let mut table = Table::new(name.to_string(), col_names);

                // Set AUTO_INCREMENT column if specified
                for col in columns {
                    if col.is_autoincrement {
                        table.set_auto_increment_column(col.name.clone());
                        break; // Only one AUTO_INCREMENT column allowed
                    }
                }

                self.tables.insert(name.to_string(), table);
                Ok(ResultSet::empty())
            }
            Statement::CreateIndex {
                name: index_name,
                table,
                columns,
                unique,
            } => {
                if let Some(t) = self.tables.get_mut(table) {
                    // For now, create a B-Tree index on the first column
                    // TODO: Support multi-column indexes
                    if let Some(first_col) = columns.first() {
                        let col_idx =
                            t.columns
                                .iter()
                                .position(|c| c == first_col)
                                .ok_or_else(|| {
                                    crate::NexusError::QueryError(format!(
                                        "Column {} not found in table {}",
                                        first_col, table
                                    ))
                                })?;

                        // Create B-Tree index
                        let mut index =
                            crate::index::btree::BTreeIndex::new(first_col.clone(), col_idx);

                        // Build index from existing rows
                        for (row_id, row) in t.rows.iter().enumerate() {
                            index.insert(row, row_id);
                        }

                        // Store index in table
                        t.indexes.insert(index_name.clone(), index);
                    }

                    Ok(ResultSet::empty())
                } else {
                    Err(crate::NexusError::QueryError(format!(
                        "Table {} not found",
                        table
                    )))
                }
            }
            Statement::DropTable { name, if_exists } => {
                if self.tables.contains_key(name) {
                    self.tables.remove(name);
                    Ok(ResultSet::empty())
                } else if *if_exists {
                    // IF EXISTS specified, don't error if table doesn't exist
                    Ok(ResultSet::empty())
                } else {
                    Err(crate::NexusError::QueryError(format!(
                        "Table '{}' does not exist",
                        name
                    )))
                }
            }
            Statement::Insert {
                table,
                columns,
                values,
            } => {
                if let Some(t) = self.tables.get_mut(table) {
                    for expr_set in values {
                        // Evaluate each expression with the provided parameters
                        // Use an empty row since we're not referencing columns in INSERT VALUES
                        let empty_row = Row::new(vec![], vec![]);
                        let evaluated_values: Vec<Value> = expr_set
                            .iter()
                            .map(|expr| evaluate_expr(&empty_row, expr, params))
                            .collect();

                        // CRITICAL FIX: Map values to correct columns based on INSERT column list
                        // If columns are specified in INSERT, use them. Otherwise use all table columns.
                        let (insert_columns, mut insert_values) =
                            if let Some(ref col_names) = columns {
                                // Columns explicitly specified: INSERT INTO table (col1, col2) VALUES (val1, val2)
                                // Map values to the correct column positions in the table
                                let mut row_values = vec![Value::Null; t.columns.len()];

                                for (i, col_name) in col_names.iter().enumerate() {
                                    if let Some(table_col_idx) =
                                        t.columns.iter().position(|c| c == col_name)
                                    {
                                        if i < evaluated_values.len() {
                                            row_values[table_col_idx] = evaluated_values[i].clone();
                                        }
                                    } else {
                                        return Err(crate::NexusError::QueryError(format!(
                                            "Column {} not found in table {}",
                                            col_name, table
                                        )));
                                    }
                                }

                                (t.columns.clone(), row_values)
                            } else {
                                // No columns specified: INSERT INTO table VALUES (val1, val2, val3)
                                // Use values positionally for all table columns
                                (t.columns.clone(), evaluated_values)
                            };

                        // AUTO_INCREMENT: Generate ID if the column is NULL
                        if let Some(ref auto_col) = t.auto_increment_column {
                            if let Some(auto_col_idx) = t.columns.iter().position(|c| c == auto_col)
                            {
                                if insert_values[auto_col_idx] == Value::Null {
                                    insert_values[auto_col_idx] =
                                        Value::Integer(t.next_auto_increment_id);
                                    t.next_auto_increment_id += 1;
                                }
                            }
                        }

                        let row = Row::new(insert_columns, insert_values);
                        t.insert(row);
                    }
                    Ok(ResultSet::empty())
                } else {
                    Err(crate::NexusError::QueryError(format!(
                        "Table {} not found",
                        table
                    )))
                }
            }
            Statement::Select {
                columns,
                from,
                where_clause,
                using_engine: _,
                order_by,
                limit,
            } => {
                let table = self.tables.get(from).ok_or_else(|| {
                    crate::NexusError::QueryError(format!("Table {} not found", from))
                })?;

                let mut rows = table.select_all();

                // Apply WHERE clause filter
                if let Some(expr) = where_clause {
                    rows.retain(|row| evaluate_where(row, expr, params));
                }

                // Check if this is an aggregate query
                if has_aggregate_functions(columns) {
                    // Handle aggregate functions - returns a single row
                    let result_row = self.evaluate_aggregates(&rows, columns, params)?;
                    return Ok(ResultSet::new(vec![result_row]));
                }

                // Apply ORDER BY sorting (especially for vector distance)
                if let Some(order) = order_by {
                    rows.sort_by(|a, b| {
                        use crate::query::evaluator::evaluate_expr;
                        let a_val = evaluate_expr(a, &order.expr, params);
                        let b_val = evaluate_expr(b, &order.expr, params);

                        let cmp = a_val
                            .partial_cmp(&b_val)
                            .unwrap_or(std::cmp::Ordering::Equal);

                        if order.ascending {
                            cmp
                        } else {
                            cmp.reverse()
                        }
                    });
                }

                // Apply LIMIT
                if let Some(lim) = limit {
                    rows.truncate(*lim);
                }

                // Project columns based on SELECT list
                let result_rows = self.project_columns(&rows, columns, params)?;

                Ok(ResultSet::new(result_rows))
            }
            Statement::Update {
                table,
                set,
                where_clause,
            } => {
                if let Some(t) = self.tables.get_mut(table) {
                    for row in &mut t.rows {
                        let should_update = where_clause
                            .as_ref()
                            .map(|expr| evaluate_where(row, expr, params))
                            .unwrap_or(true);

                        if should_update {
                            for (col_name, new_expr) in set {
                                if let Some(idx) = row.columns.iter().position(|c| c == col_name) {
                                    // Evaluate the expression with parameters
                                    row.values[idx] = evaluate_expr(row, new_expr, params);
                                }
                            }
                        }
                    }
                    Ok(ResultSet::empty())
                } else {
                    Err(crate::NexusError::QueryError(format!(
                        "Table {} not found",
                        table
                    )))
                }
            }
            Statement::Delete { from, where_clause } => {
                if let Some(t) = self.tables.get_mut(from) {
                    if let Some(expr) = where_clause {
                        t.rows.retain(|row| !evaluate_where(row, expr, params));
                    } else {
                        t.rows.clear();
                    }
                    Ok(ResultSet::empty())
                } else {
                    Err(crate::NexusError::QueryError(format!(
                        "Table {} not found",
                        from
                    )))
                }
            }
            Statement::Transaction { operation } => {
                match operation {
                    TransactionOp::Begin => {
                        if self.transaction_active {
                            return Err(crate::NexusError::QueryError(
                                "Transaction already active".into(),
                            ));
                        }
                        // Create a snapshot of current tables for potential rollback
                        self.transaction_snapshot = Some(self.tables.clone());
                        self.transaction_active = true;
                        Ok(ResultSet::empty())
                    }
                    TransactionOp::Commit => {
                        if !self.transaction_active {
                            return Err(crate::NexusError::QueryError(
                                "No active transaction to commit".into(),
                            ));
                        }
                        // Commit: discard snapshot and keep current state
                        self.transaction_snapshot = None;
                        self.transaction_active = false;
                        Ok(ResultSet::empty())
                    }
                    TransactionOp::Rollback => {
                        if !self.transaction_active {
                            return Err(crate::NexusError::QueryError(
                                "No active transaction to rollback".into(),
                            ));
                        }
                        // Rollback: restore tables from snapshot
                        if let Some(snapshot) = self.transaction_snapshot.take() {
                            self.tables = snapshot;
                        }
                        self.transaction_active = false;
                        Ok(ResultSet::empty())
                    }
                }
            }
            Statement::Pragma { name, value } => {
                // Handle common SQLite PRAGMA statements for compatibility
                match name.to_lowercase().as_str() {
                    "foreign_keys" => {
                        // Return current foreign key enforcement status
                        // For now, NexusQL doesn't enforce foreign keys, so return 0
                        let row =
                            Row::new(vec!["foreign_keys".to_string()], vec![Value::Integer(0)]);
                        Ok(ResultSet::new(vec![row]))
                    }
                    "journal_mode" => {
                        // NexusQL uses WAL-style journaling in LSM storage
                        let mode = value.as_deref().unwrap_or("WAL");
                        let row = Row::new(
                            vec!["journal_mode".to_string()],
                            vec![Value::Text(mode.to_uppercase())],
                        );
                        Ok(ResultSet::new(vec![row]))
                    }
                    "synchronous" => {
                        // Return or set synchronous mode (FULL=2, NORMAL=1, OFF=0)
                        let sync_value = if let Some(v) = value {
                            match v.to_uppercase().as_str() {
                                "FULL" => Value::Integer(2),
                                "NORMAL" => Value::Integer(1),
                                "OFF" => Value::Integer(0),
                                _ => Value::Integer(2), // Default to FULL
                            }
                        } else {
                            Value::Integer(2) // Query mode, return current (FULL)
                        };
                        let row = Row::new(vec!["synchronous".to_string()], vec![sync_value]);
                        Ok(ResultSet::new(vec![row]))
                    }
                    "table_info" => {
                        // PRAGMA table_info(table_name) - return table schema
                        if let Some(table_name) = value {
                            if let Some(table) = self.tables.get(table_name) {
                                let mut rows = Vec::new();
                                for (i, col) in table.columns.iter().enumerate() {
                                    // Format: cid, name, type, notnull, dflt_value, pk
                                    let row = Row::new(
                                        vec![
                                            "cid".to_string(),
                                            "name".to_string(),
                                            "type".to_string(),
                                            "notnull".to_string(),
                                            "dflt_value".to_string(),
                                            "pk".to_string(),
                                        ],
                                        vec![
                                            Value::Integer(i as i64),
                                            Value::Text(col.clone()),
                                            Value::Text("TEXT".to_string()), // Simplified
                                            Value::Integer(0),
                                            Value::Null,
                                            Value::Integer(0),
                                        ],
                                    );
                                    rows.push(row);
                                }
                                Ok(ResultSet::new(rows))
                            } else {
                                Err(crate::NexusError::QueryError(format!(
                                    "Table {} not found",
                                    table_name
                                )))
                            }
                        } else {
                            Err(crate::NexusError::QueryError(
                                "PRAGMA table_info requires a table name".to_string(),
                            ))
                        }
                    }
                    "user_version" => {
                        // User-defined version number for schema versioning
                        let version = value.as_deref().unwrap_or("0");
                        let row = Row::new(
                            vec!["user_version".to_string()],
                            vec![Value::Integer(version.parse::<i64>().unwrap_or(0))],
                        );
                        Ok(ResultSet::new(vec![row]))
                    }
                    "schema_version" => {
                        // Internal schema version (read-only)
                        let row =
                            Row::new(vec!["schema_version".to_string()], vec![Value::Integer(1)]);
                        Ok(ResultSet::new(vec![row]))
                    }
                    "encoding" => {
                        // NexusQL uses UTF-8 encoding
                        let row = Row::new(
                            vec!["encoding".to_string()],
                            vec![Value::Text("UTF-8".to_string())],
                        );
                        Ok(ResultSet::new(vec![row]))
                    }
                    "cache_size" => {
                        // Query cache size (in pages, -2000 means 2000KB)
                        let size = value.as_deref().unwrap_or("-2000");
                        let row = Row::new(
                            vec!["cache_size".to_string()],
                            vec![Value::Integer(size.parse::<i64>().unwrap_or(-2000))],
                        );
                        Ok(ResultSet::new(vec![row]))
                    }
                    _ => {
                        // For unsupported PRAGMA, return empty result for compatibility
                        Ok(ResultSet::empty())
                    }
                }
            }
        }
    }

    pub fn get_tables(&self) -> &HashMap<String, Table> {
        &self.tables
    }

    pub fn get_tables_mut(&mut self) -> &mut HashMap<String, Table> {
        &mut self.tables
    }

    /// Evaluate aggregate functions over a set of rows, returning a single result row
    fn evaluate_aggregates(
        &self,
        rows: &[Row],
        select_items: &[SelectItem],
        params: &QueryParams,
    ) -> Result<Row> {
        let mut result_cols = Vec::new();
        let mut result_vals = Vec::new();

        for item in select_items {
            match item {
                SelectItem::Wildcard => {
                    // Wildcard in aggregate query - unusual but handle it
                    // Just return count as a placeholder
                    result_cols.push("*".to_string());
                    result_vals.push(Value::Integer(rows.len() as i64));
                }
                SelectItem::Expr { expr, alias } => {
                    let value = if let Expr::Function { name, args } = expr {
                        if is_aggregate_expr(expr) {
                            evaluate_aggregate(rows, name, args, params)
                        } else {
                            // Non-aggregate function in aggregate query
                            // Evaluate on first row if available
                            if let Some(first_row) = rows.first() {
                                evaluate_expr(first_row, expr, params)
                            } else {
                                Value::Null
                            }
                        }
                    } else {
                        // Non-function expression in aggregate query
                        // Evaluate on first row if available
                        if let Some(first_row) = rows.first() {
                            evaluate_expr(first_row, expr, params)
                        } else {
                            Value::Null
                        }
                    };

                    // Determine column name
                    let col_name = if let Some(alias_name) = alias {
                        alias_name.clone()
                    } else {
                        match expr {
                            Expr::Column(name) => name.clone(),
                            Expr::Function { name, .. } => name.clone(),
                            _ => "expr".to_string(),
                        }
                    };

                    result_cols.push(col_name);
                    result_vals.push(value);
                }
            }
        }

        Ok(Row::new(result_cols, result_vals))
    }

    /// Project columns from rows based on SELECT list items
    fn project_columns(
        &self,
        rows: &[Row],
        select_items: &[SelectItem],
        params: &QueryParams,
    ) -> Result<Vec<Row>> {
        let mut result_rows = Vec::with_capacity(rows.len());

        for row in rows {
            let mut result_cols = Vec::new();
            let mut result_vals = Vec::new();

            for item in select_items {
                match item {
                    SelectItem::Wildcard => {
                        // Add all columns from the original row
                        result_cols.extend(row.columns.clone());
                        result_vals.extend(row.values.clone());
                    }
                    SelectItem::Expr { expr, alias } => {
                        // Evaluate the expression
                        let value = evaluate_expr(row, expr, params);

                        // Determine column name (use alias if provided, otherwise expression string)
                        let col_name = if let Some(alias_name) = alias {
                            alias_name.clone()
                        } else {
                            // Generate a name from the expression
                            match expr {
                                Expr::Column(name) => name.clone(),
                                Expr::Function { name, .. } => name.clone(),
                                _ => "expr".to_string(),
                            }
                        };

                        result_cols.push(col_name);
                        result_vals.push(value);
                    }
                }
            }

            result_rows.push(Row::new(result_cols, result_vals));
        }

        Ok(result_rows)
    }
}

// Include comprehensive tests for INSERT column mapping
#[cfg(test)]
#[path = "executor_insert_tests.rs"]
mod executor_insert_tests;

#[cfg(test)]
#[path = "executor_pragma_tests.rs"]
mod executor_pragma_tests;

#[cfg(test)]
#[path = "executor_autoincrement_tests.rs"]
mod executor_autoincrement_tests;
