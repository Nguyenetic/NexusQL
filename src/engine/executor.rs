//! SQL Execution Engine
//! PROPRIETARY

use crate::query::ast::*;
use crate::query::evaluator::{evaluate_expr, evaluate_where, QueryParams};
use crate::storage::Table;
use crate::types::{ResultSet, Row, Value};
use crate::window::{
    FrameBound, FrameType, WindowFrame as WindowFrameImpl, WindowManager,
    WindowSpec as WindowSpecImpl,
};
use crate::Result;
use ordered_float::OrderedFloat;
use std::collections::HashMap;

/// Check if a SELECT list contains window functions
fn has_window_functions(select_items: &[SelectItem]) -> bool {
    for item in select_items {
        if let SelectItem::Expr { expr, .. } = item {
            if is_window_function_expr(expr) {
                return true;
            }
        }
    }
    false
}

/// Check if an expression is a window function
fn is_window_function_expr(expr: &Expr) -> bool {
    matches!(expr, Expr::WindowFunction { .. })
}

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
        Expr::Function { name, args } => {
            let name_upper = name.to_uppercase();
            matches!(name_upper.as_str(), "COUNT" | "SUM" | "AVG" | "MIN" | "MAX")
                || (name_upper == "ST_UNION" && args.len() == 1)
        }
        _ => false,
    }
}

/// Compare two Values and return their ordering
fn compare_values(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Value::Integer(a), Value::Integer(b)) => Some(a.cmp(b)),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
        (Value::Text(a), Value::Text(b)) => Some(a.cmp(b)),
        // Cross-type comparisons for SQL compatibility
        (Value::Integer(i), Value::Float(f)) => OrderedFloat(*i as f64).partial_cmp(f),
        (Value::Float(f), Value::Integer(i)) => f.partial_cmp(&OrderedFloat(*i as f64)),
        _ => None,
    }
}

/// Evaluate an aggregate function over a set of rows
fn evaluate_aggregate(rows: &[Row], name: &str, args: &[Expr], params: &QueryParams) -> Value {
    let name_upper = name.to_uppercase();

    match name_upper.as_str() {
        "ST_UNION" if args.len() == 1 => crate::geometry::union_aggregate(
            rows.iter().map(|row| evaluate_expr(row, &args[0], params)),
        ),
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
    /// Stored relationship definitions for FETCH operations
    relationships: HashMap<String, Relationship>,
}

impl Executor {
    pub fn new() -> Self {
        Self {
            tables: HashMap::new(),
            transaction_active: false,
            transaction_snapshot: None,
            relationships: HashMap::new(),
        }
    }

    /// Get a reference to stored relationships
    pub fn get_relationships(&self) -> &HashMap<String, Relationship> {
        &self.relationships
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
                table.schema.column_types = columns
                    .iter()
                    .map(|c| (c.name.clone(), c.data_type.clone()))
                    .collect();

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
                unique: _,
                if_not_exists,
                using,
                with_options,
            } => {
                if let Some(t) = self.tables.get_mut(table) {
                    // Check IF NOT EXISTS
                    if *if_not_exists
                        && (t.indexes.contains_key(index_name)
                            || t.typed_indexes.contains_key(index_name))
                    {
                        return Ok(ResultSet::empty());
                    }

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

                        // Determine index type from USING clause
                        let index_type = using
                            .as_ref()
                            .map(|s| s.to_lowercase())
                            .unwrap_or_else(|| "btree".to_string());

                        match index_type.as_str() {
                            "hnsw" => {
                                // Parse HNSW parameters from WITH options
                                let mut m = 16usize;
                                let mut ef_construction = 200usize;
                                let mut ef_search = 50usize;

                                for opt in with_options {
                                    match opt.name.to_lowercase().as_str() {
                                        "m" => {
                                            if let Value::Integer(v) = &opt.value {
                                                m = *v as usize;
                                            }
                                        }
                                        "ef_construction" => {
                                            if let Value::Integer(v) = &opt.value {
                                                ef_construction = *v as usize;
                                            }
                                        }
                                        "ef_search" | "ef" => {
                                            if let Value::Integer(v) = &opt.value {
                                                ef_search = *v as usize;
                                            }
                                        }
                                        _ => {} // Ignore unknown options
                                    }
                                }

                                // Create HNSW index
                                let hnsw_index =
                                    crate::index::HNSWIndex::new(m, ef_construction, ef_search);

                                // Store in typed_indexes
                                t.typed_indexes.insert(
                                    index_name.clone(),
                                    crate::storage::IndexType::HNSW(hnsw_index),
                                );

                                tracing::info!(
                                    "Created HNSW index '{}' on {}.{} (m={}, ef_construction={}, ef_search={})",
                                    index_name, table, first_col, m, ef_construction, ef_search
                                );
                            }
                            "gin" => {
                                // Create GIN index for arrays/JSON/full-text
                                let gin_index = crate::index::GINIndex::new(
                                    index_name.clone(),
                                    first_col.clone(),
                                    crate::index::GINOperatorClass::TsvectorOps,
                                );

                                t.typed_indexes.insert(
                                    index_name.clone(),
                                    crate::storage::IndexType::GIN(gin_index),
                                );

                                tracing::info!(
                                    "Created GIN index '{}' on {}.{}",
                                    index_name,
                                    table,
                                    first_col
                                );
                            }
                            "gist" => {
                                // Create GiST index for geometric/spatial data
                                let index_type = if t.schema.column_types.get(first_col)
                                    == Some(&DataType::Geometry)
                                {
                                    crate::index::GiSTIndexType::RTree
                                } else {
                                    crate::index::GiSTIndexType::RangeTree
                                };
                                let mut gist_index = crate::index::GiSTIndex::new(
                                    index_name.clone(),
                                    first_col.clone(),
                                    index_type,
                                );
                                for (id, row) in t.rows.iter().enumerate() {
                                    if let Some(value) = row.values.get(col_idx) {
                                        gist_index.insert(id, value);
                                    }
                                }
                                t.schema.gist_indexes.retain(|def| def.name != *index_name);
                                t.schema.gist_indexes.push(crate::storage::GiSTDefinition {
                                    name: index_name.clone(),
                                    column: first_col.clone(),
                                    index_type,
                                });

                                t.typed_indexes.insert(
                                    index_name.clone(),
                                    crate::storage::IndexType::GiST(gist_index),
                                );

                                tracing::info!(
                                    "Created GiST index '{}' on {}.{}",
                                    index_name,
                                    table,
                                    first_col
                                );
                            }
                            "brin" => {
                                // Parse BRIN parameters
                                let mut pages_per_range = 128usize;

                                for opt in with_options {
                                    if opt.name.to_lowercase() == "pages_per_range" {
                                        if let Value::Integer(v) = &opt.value {
                                            pages_per_range = *v as usize;
                                        }
                                    }
                                }

                                // Create BRIN index for time-series data
                                let brin_index = crate::index::BRINIndex::new(
                                    index_name.clone(),
                                    first_col.clone(),
                                    Some(pages_per_range),
                                );

                                t.typed_indexes.insert(
                                    index_name.clone(),
                                    crate::storage::IndexType::BRIN(brin_index),
                                );

                                tracing::info!(
                                    "Created BRIN index '{}' on {}.{} (pages_per_range={})",
                                    index_name,
                                    table,
                                    first_col,
                                    pages_per_range
                                );
                            }
                            "btree" | _ => {
                                // Default: Create B-Tree index
                                let mut index = crate::index::btree::BTreeIndex::new(
                                    first_col.clone(),
                                    col_idx,
                                );

                                // Build index from existing rows
                                for (row_id, row) in t.rows.iter().enumerate() {
                                    index.insert(row, row_id);
                                }

                                // Store index in table (legacy storage for B-Tree)
                                t.indexes.insert(index_name.clone(), index);

                                tracing::info!(
                                    "Created B-Tree index '{}' on {}.{}",
                                    index_name,
                                    table,
                                    first_col
                                );
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
                on_conflict,
            } => {
                if let Some(t) = self.tables.get_mut(table) {
                    // Find primary key column index for conflict detection
                    let pk_col_idx = t.columns.iter().position(|c| {
                        // For now, assume first column named "id" or marked as auto_increment is PK
                        c.to_lowercase() == "id" || t.auto_increment_column.as_ref() == Some(c)
                    });

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

                        t.coerce_values(&mut insert_values)?;

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

                        // Handle ON CONFLICT / INSERT OR ... modes
                        let row = Row::new(insert_columns.clone(), insert_values.clone());

                        match on_conflict {
                            Some(crate::query::ast::OnConflict::Replace) => {
                                // INSERT OR REPLACE: Delete existing row with same PK, then insert
                                if let Some(pk_idx) = pk_col_idx {
                                    let pk_value = &insert_values[pk_idx];
                                    // Remove existing row with same PK
                                    t.rows.retain(|existing| {
                                        existing.values.get(pk_idx) != Some(pk_value)
                                    });
                                    t.rebuild_gist_indexes();
                                }
                                t.insert(row);
                            }
                            Some(crate::query::ast::OnConflict::Ignore) => {
                                // INSERT OR IGNORE: Skip if conflict
                                let has_conflict = pk_col_idx.map_or(false, |pk_idx| {
                                    let pk_value = &insert_values[pk_idx];
                                    t.rows.iter().any(|existing| {
                                        existing.values.get(pk_idx) == Some(pk_value)
                                    })
                                });
                                if !has_conflict {
                                    t.insert(row);
                                }
                            }
                            Some(crate::query::ast::OnConflict::Abort) => {
                                // INSERT OR ABORT: Fail if conflict (default behavior)
                                let has_conflict = pk_col_idx.map_or(false, |pk_idx| {
                                    let pk_value = &insert_values[pk_idx];
                                    t.rows.iter().any(|existing| {
                                        existing.values.get(pk_idx) == Some(pk_value)
                                    })
                                });
                                if has_conflict {
                                    return Err(crate::NexusError::QueryError(
                                        "UNIQUE constraint failed".to_string(),
                                    ));
                                }
                                t.insert(row);
                            }
                            Some(crate::query::ast::OnConflict::Rollback) => {
                                // INSERT OR ROLLBACK: Similar to ABORT but affects transaction
                                // For now, treat same as ABORT
                                let has_conflict = pk_col_idx.map_or(false, |pk_idx| {
                                    let pk_value = &insert_values[pk_idx];
                                    t.rows.iter().any(|existing| {
                                        existing.values.get(pk_idx) == Some(pk_value)
                                    })
                                });
                                if has_conflict {
                                    return Err(crate::NexusError::QueryError(
                                        "UNIQUE constraint failed (ROLLBACK)".to_string(),
                                    ));
                                }
                                t.insert(row);
                            }
                            Some(crate::query::ast::OnConflict::Fail) => {
                                // INSERT OR FAIL: Fail but keep previous changes
                                let has_conflict = pk_col_idx.map_or(false, |pk_idx| {
                                    let pk_value = &insert_values[pk_idx];
                                    t.rows.iter().any(|existing| {
                                        existing.values.get(pk_idx) == Some(pk_value)
                                    })
                                });
                                if has_conflict {
                                    return Err(crate::NexusError::QueryError(
                                        "UNIQUE constraint failed (FAIL)".to_string(),
                                    ));
                                }
                                t.insert(row);
                            }
                            _ => {
                                // No conflict handling or DoUpdate/DoNothing (PostgreSQL style)
                                // Just do normal insert
                                t.insert(row);
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
            Statement::Select {
                with,
                columns,
                from,
                where_clause,
                group_by,
                having,
                using_engine: _,
                order_by,
                limit,
                fetch,
            } => {
                // Execute CTEs (WITH clause) first and create temporary tables
                let cte_cleanup = self.execute_ctes(with, params)?;

                // Get primary table (may be a CTE)
                let scalar_table = Table::new(String::new(), vec![]);
                let table = if from.table.is_empty() {
                    &scalar_table
                } else {
                    self.tables.get(&from.table).ok_or_else(|| {
                        crate::NexusError::QueryError(format!("Table {} not found", from.table))
                    })?
                };

                let mut rows = if from.table.is_empty() {
                    vec![Row::new(vec![], vec![])]
                } else if from.joins.is_empty() {
                    let outer = Row::new(vec![], vec![]);
                    if let Some(ids) = where_clause.as_ref().and_then(|expr| {
                        super::spatial::candidates(
                            table,
                            from.alias.as_deref().unwrap_or(&from.table),
                            expr,
                            &outer,
                            params,
                        )
                    }) {
                        ids.into_iter()
                            .filter_map(|id| table.rows.get(id).cloned())
                            .collect()
                    } else {
                        table.select_all()
                    }
                } else {
                    table.select_all()
                };

                if from.alias.is_some() || !from.joins.is_empty() {
                    let qualifier = from.alias.as_deref().unwrap_or(&from.table);
                    rows = rows
                        .iter()
                        .map(|row| super::spatial::qualify(row, qualifier))
                        .collect();
                }

                // Execute JOINs if present
                if !from.joins.is_empty() {
                    rows = self.execute_joins(&rows, &from.joins, params)?;
                }

                // Apply WHERE clause filter with subquery support
                if let Some(expr) = where_clause {
                    if Self::contains_subqueries(expr) {
                        // Use subquery-aware evaluation
                        let mut filtered_rows = Vec::new();
                        for row in rows {
                            match self.evaluate_where_with_subqueries(&row, expr, params) {
                                Ok(true) => filtered_rows.push(row),
                                Ok(false) => {}
                                Err(_) => {} // Skip rows that cause errors in subquery evaluation
                            }
                        }
                        rows = filtered_rows;
                    } else {
                        // Use fast path for simple WHERE clauses
                        rows.retain(|row| evaluate_where(row, expr, params));
                    }
                }

                // Handle GROUP BY
                if !group_by.is_empty() {
                    // Group rows and apply aggregations
                    let grouped_rows =
                        self.execute_group_by(&rows, columns, group_by, having, params)?;

                    // Apply ORDER BY sorting on grouped results
                    let mut result_rows = grouped_rows;
                    if let Some(order) = order_by {
                        result_rows.sort_by(|a, b| {
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
                        result_rows.truncate(*lim);
                    }

                    // execute_group_by already projects columns, so return directly
                    self.cleanup_ctes(&cte_cleanup);
                    return Ok(ResultSet::new(result_rows));
                }

                // Check if this is an aggregate query (without GROUP BY)
                if has_aggregate_functions(columns) {
                    // Handle aggregate functions - returns a single row
                    let result_row = self.evaluate_aggregates(&rows, columns, params)?;
                    self.cleanup_ctes(&cte_cleanup);
                    return Ok(ResultSet::new(vec![result_row]));
                }

                // Apply ORDER BY sorting (especially for vector distance)
                if let Some(order) = order_by {
                    // SELECT aliases refer to their expressions when sorting
                    // source rows (projection happens after ORDER BY / LIMIT).
                    let order_expr = if let Expr::Column(name) = &order.expr {
                        columns
                            .iter()
                            .find_map(|item| match item {
                                SelectItem::Expr {
                                    expr,
                                    alias: Some(alias),
                                } if alias == name => Some(expr),
                                _ => None,
                            })
                            .unwrap_or(&order.expr)
                    } else {
                        &order.expr
                    };
                    // Expensive scalar scores/distances need evaluation once
                    // per row, not once per comparison during sorting.
                    if order.ascending {
                        rows.sort_by_cached_key(|row| evaluate_expr(row, order_expr, params));
                    } else {
                        rows.sort_by_cached_key(|row| {
                            std::cmp::Reverse(evaluate_expr(row, order_expr, params))
                        });
                    }
                }

                // Apply LIMIT
                if let Some(lim) = limit {
                    rows.truncate(*lim);
                }

                // Project columns based on SELECT list
                // Use window function aware projection if needed
                let mut result_rows = if has_window_functions(columns) {
                    self.project_columns_with_windows(&rows, columns, params)?
                } else {
                    self.project_columns(&rows, columns, params)?
                };

                // Execute FETCH clauses for nested data retrieval
                if !fetch.is_empty() {
                    result_rows = self.execute_fetch_clauses(&result_rows, &rows, fetch, params)?;
                }

                // Clean up CTE tables
                self.cleanup_ctes(&cte_cleanup);
                Ok(ResultSet::new(result_rows))
            }
            Statement::Update {
                table,
                set,
                where_clause,
            } => {
                if let Some(t) = self.tables.get_mut(table) {
                    // Validate the whole statement before replacing rows or index bounds.
                    let mut updated = t.rows.clone();
                    for row in &mut updated {
                        let should_update = where_clause
                            .as_ref()
                            .map(|expr| evaluate_where(row, expr, params))
                            .unwrap_or(true);

                        if should_update {
                            for (col_name, new_expr) in set {
                                if let Some(idx) = row.columns.iter().position(|c| c == col_name) {
                                    // Evaluate the expression with parameters
                                    let mut value = evaluate_expr(row, new_expr, params);
                                    if t.schema.column_types.get(col_name)
                                        == Some(&DataType::Geometry)
                                    {
                                        Table::coerce_geometry(&mut value)?;
                                    }
                                    row.values[idx] = value;
                                }
                            }
                        }
                    }
                    t.rows = updated;
                    t.rebuild_gist_indexes();
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
                    t.rebuild_gist_indexes();
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
            Statement::SetOperation {
                left,
                op,
                right,
                order_by,
                limit,
            } => {
                // Execute left and right subqueries
                let left_result = self.execute(left, params)?;
                let right_result = self.execute(right, params)?;

                // Combine results based on set operation
                let mut result_rows = match op {
                    SetOperator::Union => {
                        // UNION: combine and remove duplicates
                        let mut all_rows = left_result.rows;
                        for row in right_result.rows {
                            if !all_rows.contains(&row) {
                                all_rows.push(row);
                            }
                        }
                        all_rows
                    }
                    SetOperator::UnionAll => {
                        // UNION ALL: combine keeping duplicates
                        let mut all_rows = left_result.rows;
                        all_rows.extend(right_result.rows);
                        all_rows
                    }
                    SetOperator::Intersect => {
                        // INTERSECT: only rows in both (deduplicated)
                        let mut result = Vec::new();
                        for row in &left_result.rows {
                            if right_result.rows.contains(row) && !result.contains(row) {
                                result.push(row.clone());
                            }
                        }
                        result
                    }
                    SetOperator::IntersectAll => {
                        // INTERSECT ALL: rows in both, keeping count
                        let mut result = Vec::new();
                        let mut right_remaining: Vec<_> = right_result.rows.clone();
                        for row in &left_result.rows {
                            if let Some(pos) = right_remaining.iter().position(|r| r == row) {
                                result.push(row.clone());
                                right_remaining.remove(pos);
                            }
                        }
                        result
                    }
                    SetOperator::Except => {
                        // EXCEPT: rows in left but not in right (deduplicated)
                        let mut result = Vec::new();
                        for row in &left_result.rows {
                            if !right_result.rows.contains(row) && !result.contains(row) {
                                result.push(row.clone());
                            }
                        }
                        result
                    }
                    SetOperator::ExceptAll => {
                        // EXCEPT ALL: rows in left minus matching count from right
                        let mut result = Vec::new();
                        let mut right_remaining: Vec<_> = right_result.rows.clone();
                        for row in &left_result.rows {
                            if let Some(pos) = right_remaining.iter().position(|r| r == row) {
                                right_remaining.remove(pos); // Cancel one occurrence
                            } else {
                                result.push(row.clone());
                            }
                        }
                        result
                    }
                };

                // Apply ORDER BY to the combined result
                if let Some(order) = order_by {
                    result_rows.sort_by(|a, b| {
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
                    result_rows.truncate(*lim);
                }

                Ok(ResultSet::new(result_rows))
            }
            Statement::CreateRelationship(rel) => {
                // Store relationship definition for FETCH operations
                self.relationships.insert(rel.name.clone(), rel.clone());
                Ok(ResultSet::empty())
            }
            Statement::Traverse {
                clause,
                from_table,
                where_clause,
                return_columns,
                limit,
            } => {
                // Execute graph traversal
                self.execute_traverse(
                    clause,
                    from_table,
                    where_clause,
                    return_columns,
                    limit,
                    params,
                )
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
                        result_cols.extend(
                            row.columns
                                .iter()
                                .map(|c| c.rsplit('.').next().unwrap_or(c).to_string()),
                        );
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

    /// Project columns with window function support
    /// Pre-computes window function values before projection
    fn project_columns_with_windows(
        &self,
        rows: &[Row],
        select_items: &[SelectItem],
        params: &QueryParams,
    ) -> Result<Vec<Row>> {
        // Pre-compute all window function values
        let window_values = self.compute_window_functions(rows, select_items)?;

        let mut result_rows = Vec::with_capacity(rows.len());

        for (row_idx, row) in rows.iter().enumerate() {
            let mut result_cols = Vec::new();
            let mut result_vals = Vec::new();
            let mut window_col_idx = 0;

            for item in select_items {
                match item {
                    SelectItem::Wildcard => {
                        result_cols.extend(row.columns.clone());
                        result_vals.extend(row.values.clone());
                    }
                    SelectItem::Expr { expr, alias } => {
                        let value = if is_window_function_expr(expr) {
                            // Use pre-computed window function value
                            let val = window_values
                                .get(window_col_idx)
                                .and_then(|col_vals| col_vals.get(row_idx))
                                .cloned()
                                .unwrap_or(Value::Null);
                            window_col_idx += 1;
                            val
                        } else {
                            evaluate_expr(row, expr, params)
                        };

                        let col_name = if let Some(alias_name) = alias {
                            alias_name.clone()
                        } else {
                            self.get_window_column_name(expr)
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

    /// Get column name for a window function expression
    fn get_window_column_name(&self, expr: &Expr) -> String {
        match expr {
            Expr::Column(name) => name.clone(),
            Expr::Function { name, .. } => name.clone(),
            Expr::WindowFunction { function, .. } => {
                if let Expr::Function { name, .. } = function.as_ref() {
                    name.clone()
                } else {
                    "window_func".to_string()
                }
            }
            _ => "expr".to_string(),
        }
    }

    /// Compute all window functions in the SELECT list
    fn compute_window_functions(
        &self,
        rows: &[Row],
        select_items: &[SelectItem],
    ) -> Result<Vec<Vec<Value>>> {
        let manager = WindowManager::new();
        let mut window_results = Vec::new();

        for item in select_items {
            if let SelectItem::Expr { expr, .. } = item {
                if let Expr::WindowFunction {
                    function,
                    window_spec,
                } = expr
                {
                    // Convert AST WindowSpec to window module WindowSpec
                    let impl_spec = self.convert_window_spec(window_spec, rows);

                    // Get function name and value column
                    let (func_name, value_col_idx) =
                        self.extract_window_function_info(function, rows);

                    // For RANK and DENSE_RANK, use ORDER BY column as value_column if not set
                    let effective_value_col = if value_col_idx.is_none()
                        && (func_name == "RANK" || func_name == "DENSE_RANK")
                    {
                        // Use the first ORDER BY column
                        impl_spec.order_by.first().map(|(idx, _)| *idx)
                    } else {
                        value_col_idx
                    };

                    // Execute window function
                    let values = manager.execute_window_function(
                        &func_name,
                        rows,
                        &impl_spec,
                        effective_value_col,
                    )?;

                    window_results.push(values);
                }
            }
        }

        Ok(window_results)
    }

    /// Convert AST WindowSpec to window module WindowSpec
    fn convert_window_spec(&self, spec: &WindowSpec, rows: &[Row]) -> WindowSpecImpl {
        // Convert PARTITION BY expressions to column indices
        let partition_by: Vec<usize> = spec
            .partition_by
            .iter()
            .filter_map(|expr| self.expr_to_column_idx(expr, rows))
            .collect();

        // Convert ORDER BY expressions to column indices and direction
        let order_by: Vec<(usize, bool)> = spec
            .order_by
            .iter()
            .filter_map(|order| {
                self.expr_to_column_idx(&order.expr, rows)
                    .map(|idx| (idx, order.ascending))
            })
            .collect();

        // Convert window frame if present
        let frame = if let Some(ref ast_frame) = spec.frame {
            WindowFrameImpl::new(
                match ast_frame.units {
                    WindowFrameUnits::Rows => FrameType::Rows,
                    WindowFrameUnits::Range => FrameType::Range,
                    WindowFrameUnits::Groups => FrameType::Rows, // Treat GROUPS as ROWS for now
                },
                self.convert_frame_bound(&ast_frame.start),
                ast_frame
                    .end
                    .as_ref()
                    .map(|e| self.convert_frame_bound(e))
                    .unwrap_or(FrameBound::CurrentRow),
            )
        } else {
            WindowFrameImpl::default()
        };

        WindowSpecImpl {
            partition_by,
            order_by,
            frame,
        }
    }

    /// Convert AST WindowFrameBound to window module FrameBound
    fn convert_frame_bound(&self, bound: &WindowFrameBound) -> FrameBound {
        match bound {
            WindowFrameBound::CurrentRow => FrameBound::CurrentRow,
            WindowFrameBound::UnboundedPreceding => FrameBound::UnboundedPreceding,
            WindowFrameBound::UnboundedFollowing => FrameBound::UnboundedFollowing,
            WindowFrameBound::Preceding(n) => FrameBound::Preceding(*n as i64),
            WindowFrameBound::Following(n) => FrameBound::Following(*n as i64),
        }
    }

    /// Extract function name and value column index from window function
    fn extract_window_function_info(
        &self,
        function: &Expr,
        rows: &[Row],
    ) -> (String, Option<usize>) {
        match function {
            Expr::Function { name, args } => {
                let func_name = name.to_uppercase();
                let value_col = args
                    .first()
                    .and_then(|arg| self.expr_to_column_idx(arg, rows));
                (func_name, value_col)
            }
            _ => ("ROW_NUMBER".to_string(), None),
        }
    }

    /// Convert an expression to a column index (if it's a simple column reference)
    fn expr_to_column_idx(&self, expr: &Expr, rows: &[Row]) -> Option<usize> {
        if let Expr::Column(col_name) = expr {
            if let Some(first_row) = rows.first() {
                return first_row.columns.iter().position(|c| c == col_name);
            }
        }
        None
    }

    /// Execute CTEs (WITH clause) by materializing them as temporary tables
    /// Returns a list of CTE names that should be cleaned up after the main query
    fn execute_ctes(
        &mut self,
        ctes: &[crate::query::ast::CommonTableExpr],
        params: &QueryParams,
    ) -> Result<Vec<String>> {
        use crate::query::ast::SelectItem;

        let mut cte_names = Vec::new();

        for cte in ctes {
            // Materialize the CTE as a temporary table
            let mut rows = self.execute_subquery_expr(&cte.query, params)?;

            // Determine column names for the CTE
            let columns = if let Some(ref col_names) = cte.columns {
                // Use provided column names and rename row columns
                for row in &mut rows {
                    if row.columns.len() == col_names.len() {
                        row.columns = col_names.clone();
                    }
                }
                col_names.clone()
            } else {
                // Infer column names from the subquery columns or from the rows
                if !rows.is_empty() {
                    rows[0].columns.clone()
                } else {
                    cte.query
                        .columns
                        .iter()
                        .enumerate()
                        .map(|(i, item)| match item {
                            SelectItem::Wildcard => format!("col{}", i),
                            SelectItem::Expr { alias, expr } => {
                                alias.clone().unwrap_or_else(|| match expr {
                                    crate::query::ast::Expr::Column(name) => name.clone(),
                                    _ => format!("col{}", i),
                                })
                            }
                        })
                        .collect()
                }
            };

            let mut table = Table::new(cte.name.clone(), columns);
            table.rows = rows;

            self.tables.insert(cte.name.clone(), table);
            cte_names.push(cte.name.clone());
        }

        Ok(cte_names)
    }

    /// Clean up temporary CTE tables
    fn cleanup_ctes(&mut self, cte_names: &[String]) {
        for name in cte_names {
            self.tables.remove(name);
        }
    }

    /// Execute a SubqueryExpr and return the result rows
    fn execute_subquery_expr(
        &self,
        subquery: &crate::query::ast::SubqueryExpr,
        params: &QueryParams,
    ) -> Result<Vec<Row>> {
        use crate::query::ast::TableRef;
        use crate::query::evaluator::evaluate_where;

        // Get rows from the FROM clause
        let mut rows = match &subquery.from {
            TableRef::Table { name, .. } => {
                let table = self.tables.get(name).ok_or_else(|| {
                    crate::NexusError::QueryError(format!("Table {} not found", name))
                })?;
                table.select_all()
            }
            TableRef::Subquery { .. } => {
                // TODO: Support derived tables (subqueries in FROM)
                return Err(crate::NexusError::QueryError(
                    "Derived tables (subqueries in FROM) not yet supported".into(),
                ));
            }
        };

        // Apply WHERE clause
        if let Some(ref where_expr) = subquery.where_clause {
            rows.retain(|row| evaluate_where(row, where_expr, params));
        }

        // TODO: Handle GROUP BY and HAVING in subqueries

        // Apply ORDER BY
        if let Some(ref order) = subquery.order_by {
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
        if let Some(lim) = subquery.limit {
            rows.truncate(lim);
        }

        // Project columns
        let projected = self.project_columns(&rows, &subquery.columns, params)?;

        Ok(projected)
    }

    /// Evaluate a WHERE clause with full subquery support
    /// This method handles IN subqueries, EXISTS subqueries, and scalar subqueries
    fn evaluate_where_with_subqueries(
        &self,
        row: &Row,
        expr: &Expr,
        params: &QueryParams,
    ) -> Result<bool> {
        match expr {
            Expr::InSubquery {
                expr: value_expr,
                subquery,
                negated,
            } => {
                // Execute the subquery to get the set of values
                let subquery_rows = self.execute_subquery_expr(subquery, params)?;

                // Get the value to check
                let value = evaluate_expr(row, value_expr, params);

                // Check if value is in the subquery results (first column of each row)
                let found = subquery_rows.iter().any(|sub_row| {
                    if let Some(first_val) = sub_row.values.first() {
                        *first_val == value
                    } else {
                        false
                    }
                });

                Ok(if *negated { !found } else { found })
            }

            Expr::ExistsSubquery { subquery, negated } => {
                // For EXISTS, we need to execute the subquery with potential correlation
                // The subquery's WHERE clause might reference the outer row
                let subquery_rows = self.execute_correlated_subquery(subquery, row, params)?;

                // EXISTS returns true if the subquery returns any rows
                let exists = !subquery_rows.is_empty();

                Ok(if *negated { !exists } else { exists })
            }

            Expr::BinaryOp { left, op, right } => {
                // Handle AND/OR with subquery support
                match op {
                    BinaryOperator::And => {
                        let left_result = self.evaluate_where_with_subqueries(row, left, params)?;
                        if !left_result {
                            return Ok(false);
                        }
                        self.evaluate_where_with_subqueries(row, right, params)
                    }
                    BinaryOperator::Or => {
                        let left_result = self.evaluate_where_with_subqueries(row, left, params)?;
                        if left_result {
                            return Ok(true);
                        }
                        self.evaluate_where_with_subqueries(row, right, params)
                    }
                    _ => {
                        // For comparison operators, evaluate both sides and compare
                        let left_val = self.evaluate_expr_with_subqueries(row, left, params)?;
                        let right_val = self.evaluate_expr_with_subqueries(row, right, params)?;
                        Ok(self.evaluate_comparison(&left_val, op, &right_val))
                    }
                }
            }

            // For all other expressions, use the standard evaluator
            _ => Ok(evaluate_where(row, expr, params)),
        }
    }

    /// Evaluate an expression that may contain subqueries
    fn evaluate_expr_with_subqueries(
        &self,
        row: &Row,
        expr: &Expr,
        params: &QueryParams,
    ) -> Result<Value> {
        match expr {
            Expr::Subquery(subquery) => {
                // Scalar subquery - execute and return the first value
                let subquery_rows = self.execute_subquery_expr(subquery, params)?;

                if let Some(first_row) = subquery_rows.first() {
                    if let Some(first_val) = first_row.values.first() {
                        return Ok(first_val.clone());
                    }
                }

                Ok(Value::Null)
            }

            Expr::InSubquery {
                expr: value_expr,
                subquery,
                negated,
            } => {
                // Evaluate as boolean
                let subquery_rows = self.execute_subquery_expr(subquery, params)?;
                let value = evaluate_expr(row, value_expr, params);

                let found = subquery_rows.iter().any(|sub_row| {
                    if let Some(first_val) = sub_row.values.first() {
                        *first_val == value
                    } else {
                        false
                    }
                });

                Ok(Value::Boolean(if *negated { !found } else { found }))
            }

            Expr::ExistsSubquery { subquery, negated } => {
                let subquery_rows = self.execute_correlated_subquery(subquery, row, params)?;
                let exists = !subquery_rows.is_empty();
                Ok(Value::Boolean(if *negated { !exists } else { exists }))
            }

            // For all other expressions, use the standard evaluator
            _ => Ok(evaluate_expr(row, expr, params)),
        }
    }

    /// Execute a potentially correlated subquery
    /// The subquery's WHERE clause might reference columns from the outer row
    fn execute_correlated_subquery(
        &self,
        subquery: &SubqueryExpr,
        outer_row: &Row,
        params: &QueryParams,
    ) -> Result<Vec<Row>> {
        use crate::query::ast::TableRef;

        // Get rows from the FROM clause
        let mut rows = match &subquery.from {
            TableRef::Table { name, .. } => {
                let table = self.tables.get(name).ok_or_else(|| {
                    crate::NexusError::QueryError(format!("Table {} not found", name))
                })?;
                table.select_all()
            }
            TableRef::Subquery { .. } => {
                return Err(crate::NexusError::QueryError(
                    "Derived tables in correlated subqueries not yet supported".into(),
                ));
            }
        };

        // Apply WHERE clause with correlation to outer row
        if let Some(ref where_expr) = subquery.where_clause {
            rows.retain(|inner_row| {
                // Create a merged row that has both inner and outer columns
                // This allows the WHERE clause to reference outer row columns
                let merged_row = self.merge_correlated_rows(outer_row, inner_row);
                evaluate_where(&merged_row, where_expr, params)
            });
        }

        // Apply LIMIT
        if let Some(lim) = subquery.limit {
            rows.truncate(lim);
        }

        // Project columns
        let projected = self.project_columns(&rows, &subquery.columns, params)?;

        Ok(projected)
    }

    /// Merge outer and inner rows for correlated subquery evaluation
    /// Outer row columns are accessible in the subquery's WHERE clause
    /// Inner row columns are added without replacing outer columns
    fn merge_correlated_rows(&self, outer_row: &Row, inner_row: &Row) -> Row {
        let mut columns = outer_row.columns.clone();
        let mut values = outer_row.values.clone();

        // Add inner row columns WITHOUT replacing outer columns
        // This allows the WHERE clause to reference outer columns like users.id
        // while also accessing inner columns like orders.user_id
        for (i, col) in inner_row.columns.iter().enumerate() {
            // Only add if the column doesn't exist (don't overwrite outer columns)
            if !columns.contains(col) {
                columns.push(col.clone());
                values.push(inner_row.values[i].clone());
            }
        }

        Row::new(columns, values)
    }

    /// Evaluate a comparison operation between two values
    fn evaluate_comparison(&self, left: &Value, op: &BinaryOperator, right: &Value) -> bool {
        match op {
            BinaryOperator::Eq => left == right,
            BinaryOperator::NotEq => left != right,
            BinaryOperator::Lt => compare_values(left, right)
                .map(|c| c == std::cmp::Ordering::Less)
                .unwrap_or(false),
            BinaryOperator::LtEq => compare_values(left, right)
                .map(|c| c != std::cmp::Ordering::Greater)
                .unwrap_or(false),
            BinaryOperator::Gt => compare_values(left, right)
                .map(|c| c == std::cmp::Ordering::Greater)
                .unwrap_or(false),
            BinaryOperator::GtEq => compare_values(left, right)
                .map(|c| c != std::cmp::Ordering::Less)
                .unwrap_or(false),
            _ => false,
        }
    }

    /// Check if a WHERE expression contains subqueries that need special handling
    fn contains_subqueries(expr: &Expr) -> bool {
        match expr {
            Expr::InSubquery { .. } => true,
            Expr::ExistsSubquery { .. } => true,
            Expr::Subquery(_) => true,
            Expr::BinaryOp { left, right, .. } => {
                Self::contains_subqueries(left) || Self::contains_subqueries(right)
            }
            _ => false,
        }
    }

    /// Execute JOINs using nested loop algorithm
    fn execute_joins(
        &self,
        left_rows: &[Row],
        joins: &[JoinClause],
        params: &QueryParams,
    ) -> Result<Vec<Row>> {
        let mut result_rows = left_rows.to_vec();

        for join in joins {
            result_rows = self.execute_single_join(&result_rows, join, params)?;
        }

        Ok(result_rows)
    }

    /// Execute a single JOIN operation
    fn execute_single_join(
        &self,
        left_rows: &[Row],
        join: &JoinClause,
        params: &QueryParams,
    ) -> Result<Vec<Row>> {
        // Get the right table
        let right_table = self.tables.get(&join.table).ok_or_else(|| {
            crate::NexusError::QueryError(format!("Table {} not found for JOIN", join.table))
        })?;
        let qualifier = join.alias.as_deref().unwrap_or(&join.table);
        let right_rows: Vec<_> = right_table
            .rows
            .iter()
            .map(|row| super::spatial::qualify(row, qualifier))
            .collect();

        // Parameterized index nested loop. Each outer row supplies the geometry
        // for one R-tree probe; every candidate still goes through the exact ON.
        if matches!(join.join_type, JoinType::Inner | JoinType::Left) {
            let mut result = Vec::new();
            for left in left_rows {
                let ids = join.on_condition.as_ref().and_then(|expr| {
                    super::spatial::candidates(right_table, qualifier, expr, left, params)
                });
                let rows: Box<dyn Iterator<Item = &Row> + '_> = if let Some(ids) = ids {
                    Box::new(ids.into_iter().filter_map(|id| right_rows.get(id)))
                } else {
                    Box::new(right_rows.iter())
                };
                let mut matched = false;
                for right in rows {
                    if self.evaluate_join_condition(left, right, &join.on_condition, params) {
                        result.push(self.merge_rows(left, right, &None));
                        matched = true;
                    }
                }
                if !matched && join.join_type == JoinType::Left {
                    result.push(self.merge_rows_with_nulls(
                        left,
                        right_table,
                        &Some(qualifier.to_string()),
                    ));
                }
            }
            return Ok(result);
        }

        let mut result = Vec::new();

        match join.join_type {
            JoinType::Inner => {
                // INNER JOIN: only matching rows
                for left_row in left_rows {
                    for right_row in &right_rows {
                        if self.evaluate_join_condition(
                            left_row,
                            right_row,
                            &join.on_condition,
                            params,
                        ) {
                            result.push(self.merge_rows(left_row, right_row, &join.alias));
                        }
                    }
                }
            }
            JoinType::Left => {
                // LEFT JOIN: all left rows + matching right rows (NULL for non-matches)
                for left_row in left_rows {
                    let mut matched = false;
                    for right_row in &right_rows {
                        if self.evaluate_join_condition(
                            left_row,
                            right_row,
                            &join.on_condition,
                            params,
                        ) {
                            result.push(self.merge_rows(left_row, right_row, &join.alias));
                            matched = true;
                        }
                    }
                    if !matched {
                        // Add left row with NULL values for right table columns
                        result.push(self.merge_rows_with_nulls(left_row, right_table, &join.alias));
                    }
                }
            }
            JoinType::Right => {
                // RIGHT JOIN: all right rows + matching left rows (NULL for non-matches)
                for right_row in &right_rows {
                    let mut matched = false;
                    for left_row in left_rows {
                        if self.evaluate_join_condition(
                            left_row,
                            right_row,
                            &join.on_condition,
                            params,
                        ) {
                            result.push(self.merge_rows(left_row, right_row, &join.alias));
                            matched = true;
                        }
                    }
                    if !matched {
                        // Add right row with NULL values for left table columns
                        // For right join, we need the left columns as NULL
                        if let Some(first_left) = left_rows.first() {
                            result.push(self.merge_rows_left_nulls(
                                first_left,
                                right_row,
                                &join.alias,
                            ));
                        }
                    }
                }
            }
            JoinType::Full => {
                // FULL OUTER JOIN: all rows from both tables
                let mut matched_right_indices = std::collections::HashSet::new();

                for left_row in left_rows {
                    let mut matched = false;
                    for (right_idx, right_row) in right_rows.iter().enumerate() {
                        if self.evaluate_join_condition(
                            left_row,
                            right_row,
                            &join.on_condition,
                            params,
                        ) {
                            result.push(self.merge_rows(left_row, right_row, &join.alias));
                            matched = true;
                            matched_right_indices.insert(right_idx);
                        }
                    }
                    if !matched {
                        result.push(self.merge_rows_with_nulls(left_row, right_table, &join.alias));
                    }
                }

                // Add unmatched right rows
                if let Some(first_left) = left_rows.first() {
                    for (right_idx, right_row) in right_rows.iter().enumerate() {
                        if !matched_right_indices.contains(&right_idx) {
                            result.push(self.merge_rows_left_nulls(
                                first_left,
                                right_row,
                                &join.alias,
                            ));
                        }
                    }
                }
            }
            JoinType::Cross => {
                // CROSS JOIN: cartesian product (no condition)
                for left_row in left_rows {
                    for right_row in &right_rows {
                        result.push(self.merge_rows(left_row, right_row, &join.alias));
                    }
                }
            }
        }

        Ok(result)
    }

    /// Evaluate JOIN condition
    fn evaluate_join_condition(
        &self,
        left_row: &Row,
        right_row: &Row,
        condition: &Option<Expr>,
        params: &QueryParams,
    ) -> bool {
        match condition {
            Some(expr) => {
                // Create a merged row for evaluation
                let merged = self.merge_rows(left_row, right_row, &None);
                evaluate_where(&merged, expr, params)
            }
            None => true, // No condition = all rows match (for CROSS JOIN)
        }
    }

    /// Merge two rows into one (for JOIN results)
    fn merge_rows(&self, left: &Row, right: &Row, right_alias: &Option<String>) -> Row {
        let mut columns = left.columns.clone();
        let mut values = left.values.clone();

        // Add right table columns with optional alias prefix
        for (i, col) in right.columns.iter().enumerate() {
            let col_name = if col.contains('.') {
                col.clone()
            } else if let Some(alias) = right_alias {
                format!("{}.{}", alias, col)
            } else {
                // Prefix with table name to avoid conflicts
                col.clone()
            };
            columns.push(col_name);
            values.push(right.values[i].clone());
        }

        Row::new(columns, values)
    }

    /// Merge left row with NULL values for right table columns (for LEFT JOIN)
    fn merge_rows_with_nulls(
        &self,
        left: &Row,
        right_table: &Table,
        right_alias: &Option<String>,
    ) -> Row {
        let mut columns = left.columns.clone();
        let mut values = left.values.clone();

        // Add right table columns with NULL values
        for col_name in &right_table.columns {
            let col = if let Some(alias) = right_alias {
                format!("{}.{}", alias, col_name)
            } else {
                col_name.clone()
            };
            columns.push(col);
            values.push(Value::Null);
        }

        Row::new(columns, values)
    }

    /// Merge right row with NULL values for left table columns (for RIGHT JOIN)
    fn merge_rows_left_nulls(
        &self,
        left_template: &Row,
        right: &Row,
        right_alias: &Option<String>,
    ) -> Row {
        let mut columns = Vec::new();
        let mut values = Vec::new();

        // Add left table columns with NULL values
        for col in &left_template.columns {
            columns.push(col.clone());
            values.push(Value::Null);
        }

        // Add right table columns with actual values
        for (i, col) in right.columns.iter().enumerate() {
            let col_name = if col.contains('.') {
                col.clone()
            } else if let Some(alias) = right_alias {
                format!("{}.{}", alias, col)
            } else {
                col.clone()
            };
            columns.push(col_name);
            values.push(right.values[i].clone());
        }

        Row::new(columns, values)
    }

    /// Execute GROUP BY clause
    fn execute_group_by(
        &self,
        rows: &[Row],
        columns: &[SelectItem],
        group_by: &[Expr],
        having: &Option<Expr>,
        params: &QueryParams,
    ) -> Result<Vec<Row>> {
        use crate::query::evaluator::evaluate_expr;
        use std::collections::HashMap;

        // Group rows by GROUP BY expressions
        let mut groups: HashMap<Vec<Value>, Vec<Row>> = HashMap::new();

        for row in rows {
            // Compute the group key from GROUP BY expressions
            let group_key: Vec<Value> = group_by
                .iter()
                .map(|expr| evaluate_expr(row, expr, params))
                .collect();

            groups
                .entry(group_key)
                .or_insert_with(Vec::new)
                .push(row.clone());
        }

        // Process each group
        let mut result_rows = Vec::new();

        for (group_key, group_rows) in groups {
            // Apply HAVING filter first (before building result row)
            if let Some(having_expr) = having {
                if !self.evaluate_having_expr(having_expr, &group_rows, params)? {
                    continue; // Skip this group
                }
            }

            // Evaluate aggregates and columns for this group
            let result_row =
                self.evaluate_group_row(&group_rows, columns, group_by, &group_key, params)?;

            result_rows.push(result_row);
        }

        Ok(result_rows)
    }

    /// Evaluate a HAVING expression against a group of rows
    fn evaluate_having_expr(
        &self,
        expr: &Expr,
        group_rows: &[Row],
        params: &QueryParams,
    ) -> Result<bool> {
        let value = self.evaluate_having_value(expr, group_rows, params)?;
        match value {
            Value::Boolean(b) => Ok(b),
            _ => Ok(false),
        }
    }

    /// Evaluate a HAVING expression to get its value
    fn evaluate_having_value(
        &self,
        expr: &Expr,
        group_rows: &[Row],
        params: &QueryParams,
    ) -> Result<Value> {
        use crate::query::evaluator::evaluate_expr;

        match expr {
            Expr::Function { name, args } => {
                // Evaluate aggregate function over the group
                self.evaluate_aggregate_for_group(name, args, group_rows, params)
            }
            Expr::BinaryOp { left, op, right } => {
                let left_val = self.evaluate_having_value(left, group_rows, params)?;
                let right_val = self.evaluate_having_value(right, group_rows, params)?;

                let result = match op {
                    BinaryOperator::Gt => compare_values(&left_val, &right_val)
                        .map(|c| c == std::cmp::Ordering::Greater)
                        .unwrap_or(false),
                    BinaryOperator::GtEq => compare_values(&left_val, &right_val)
                        .map(|c| c != std::cmp::Ordering::Less)
                        .unwrap_or(false),
                    BinaryOperator::Lt => compare_values(&left_val, &right_val)
                        .map(|c| c == std::cmp::Ordering::Less)
                        .unwrap_or(false),
                    BinaryOperator::LtEq => compare_values(&left_val, &right_val)
                        .map(|c| c != std::cmp::Ordering::Greater)
                        .unwrap_or(false),
                    BinaryOperator::Eq => left_val == right_val,
                    BinaryOperator::NotEq => left_val != right_val,
                    BinaryOperator::And => {
                        matches!(
                            (&left_val, &right_val),
                            (Value::Boolean(true), Value::Boolean(true))
                        )
                    }
                    BinaryOperator::Or => {
                        matches!(&left_val, Value::Boolean(true))
                            || matches!(&right_val, Value::Boolean(true))
                    }
                    _ => false,
                };

                Ok(Value::Boolean(result))
            }
            Expr::Literal(val) => Ok(val.clone()),
            Expr::Column(_col_name) => {
                // For columns in HAVING, use the first row's value
                if let Some(first_row) = group_rows.first() {
                    Ok(evaluate_expr(first_row, expr, params))
                } else {
                    Ok(Value::Null)
                }
            }
            _ => {
                // For other expressions, evaluate against first row
                if let Some(first_row) = group_rows.first() {
                    Ok(evaluate_expr(first_row, expr, params))
                } else {
                    Ok(Value::Null)
                }
            }
        }
    }

    /// Evaluate a single group row (for GROUP BY)
    fn evaluate_group_row(
        &self,
        group_rows: &[Row],
        columns: &[SelectItem],
        group_by: &[Expr],
        group_key: &[Value],
        params: &QueryParams,
    ) -> Result<Row> {
        use crate::query::evaluator::evaluate_expr;

        let mut result_columns = Vec::new();
        let mut result_values = Vec::new();

        // Use the first row as a reference for non-aggregate columns
        let first_row = group_rows
            .first()
            .ok_or_else(|| crate::NexusError::QueryError("Empty group in GROUP BY".to_string()))?;

        for item in columns {
            match item {
                SelectItem::Wildcard => {
                    // For wildcard with GROUP BY, use values from first row
                    for (col, val) in first_row.columns.iter().zip(first_row.values.iter()) {
                        result_columns.push(col.clone());
                        result_values.push(val.clone());
                    }
                }
                SelectItem::Expr { expr, alias } => {
                    let col_name = if let Some(a) = alias {
                        a.clone()
                    } else {
                        match expr {
                            Expr::Column(name) => name.clone(),
                            Expr::Function { name, .. } => name.clone(),
                            _ => "?column?".to_string(),
                        }
                    };

                    let value = match expr {
                        Expr::Function { name, args } => {
                            // Handle aggregate functions over the group
                            self.evaluate_aggregate_for_group(name, args, group_rows, params)?
                        }
                        Expr::Column(col_name) => {
                            // For non-aggregate columns in GROUP BY, check if it's in the group key
                            let expr_in_group = group_by.iter().enumerate().find(|(_, gb_expr)| {
                                matches!(gb_expr, Expr::Column(gc) if gc == col_name)
                            });

                            if let Some((idx, _)) = expr_in_group {
                                group_key.get(idx).cloned().unwrap_or(Value::Null)
                            } else {
                                // Use value from first row (behavior matches most SQL databases)
                                evaluate_expr(first_row, expr, params)
                            }
                        }
                        _ => evaluate_expr(first_row, expr, params),
                    };

                    result_columns.push(col_name);
                    result_values.push(value);
                }
            }
        }

        Ok(Row::new(result_columns, result_values))
    }

    /// Evaluate an aggregate function for a group of rows
    fn evaluate_aggregate_for_group(
        &self,
        func_name: &str,
        args: &[Expr],
        group_rows: &[Row],
        params: &QueryParams,
    ) -> Result<Value> {
        use crate::query::evaluator::evaluate_expr;

        let func_upper = func_name.to_uppercase();

        match func_upper.as_str() {
            "ST_UNION" if args.len() == 1 => {
                Ok(evaluate_aggregate(group_rows, func_name, args, params))
            }
            "COUNT" => {
                if args.len() == 1 {
                    if matches!(&args[0], Expr::Wildcard) {
                        // COUNT(*) - count all rows in group
                        Ok(Value::Integer(group_rows.len() as i64))
                    } else {
                        // COUNT(column) - count non-null values
                        let count = group_rows
                            .iter()
                            .filter(|row| {
                                let val = evaluate_expr(row, &args[0], params);
                                !matches!(val, Value::Null)
                            })
                            .count();
                        Ok(Value::Integer(count as i64))
                    }
                } else {
                    Ok(Value::Integer(group_rows.len() as i64))
                }
            }
            "SUM" => {
                if args.is_empty() {
                    return Err(crate::NexusError::QueryError(
                        "SUM requires an argument".to_string(),
                    ));
                }

                let mut sum: f64 = 0.0;
                for row in group_rows {
                    match evaluate_expr(row, &args[0], params) {
                        Value::Integer(i) => sum += i as f64,
                        Value::Float(f) => sum += f.into_inner(),
                        Value::Null => {} // Skip nulls
                        _ => {}
                    }
                }
                Ok(Value::Float(ordered_float::OrderedFloat(sum)))
            }
            "AVG" => {
                if args.is_empty() {
                    return Err(crate::NexusError::QueryError(
                        "AVG requires an argument".to_string(),
                    ));
                }

                let mut sum: f64 = 0.0;
                let mut count = 0;
                for row in group_rows {
                    match evaluate_expr(row, &args[0], params) {
                        Value::Integer(i) => {
                            sum += i as f64;
                            count += 1;
                        }
                        Value::Float(f) => {
                            sum += f.into_inner();
                            count += 1;
                        }
                        Value::Null => {} // Skip nulls
                        _ => {}
                    }
                }

                if count > 0 {
                    Ok(Value::Float(ordered_float::OrderedFloat(
                        sum / count as f64,
                    )))
                } else {
                    Ok(Value::Null)
                }
            }
            "MIN" => {
                if args.is_empty() {
                    return Err(crate::NexusError::QueryError(
                        "MIN requires an argument".to_string(),
                    ));
                }

                let mut min: Option<Value> = None;
                for row in group_rows {
                    let val = evaluate_expr(row, &args[0], params);
                    if matches!(val, Value::Null) {
                        continue;
                    }
                    min = Some(match &min {
                        None => val,
                        Some(current) => {
                            if val < *current {
                                val
                            } else {
                                current.clone()
                            }
                        }
                    });
                }
                Ok(min.unwrap_or(Value::Null))
            }
            "MAX" => {
                if args.is_empty() {
                    return Err(crate::NexusError::QueryError(
                        "MAX requires an argument".to_string(),
                    ));
                }

                let mut max: Option<Value> = None;
                for row in group_rows {
                    let val = evaluate_expr(row, &args[0], params);
                    if matches!(val, Value::Null) {
                        continue;
                    }
                    max = Some(match &max {
                        None => val,
                        Some(current) => {
                            if val > *current {
                                val
                            } else {
                                current.clone()
                            }
                        }
                    });
                }
                Ok(max.unwrap_or(Value::Null))
            }
            _ => {
                // Unknown aggregate, evaluate on first row
                use crate::query::evaluator::evaluate_expr;
                let first_row = group_rows
                    .first()
                    .ok_or_else(|| crate::NexusError::QueryError("Empty group".to_string()))?;
                Ok(evaluate_expr(
                    first_row,
                    &Expr::Function {
                        name: func_name.to_string(),
                        args: args.to_vec(),
                    },
                    params,
                ))
            }
        }
    }

    /// Execute FETCH clauses to retrieve nested related data
    fn execute_fetch_clauses(
        &self,
        result_rows: &[Row],
        original_rows: &[Row],
        fetch_clauses: &[FetchClause],
        params: &QueryParams,
    ) -> Result<Vec<Row>> {
        let mut enriched_rows = result_rows.to_vec();

        for (row_idx, orig_row) in original_rows.iter().enumerate() {
            if row_idx >= enriched_rows.len() {
                break;
            }

            for fetch in fetch_clauses {
                // Look up the relationship (either by name or treat as direct table reference)
                let related_data = self.fetch_related_data(orig_row, fetch, params)?;

                // Add the fetched data as a new column (JSON-serialized)
                let fetch_col_name = format!("_{}", fetch.relationship);
                enriched_rows[row_idx].columns.push(fetch_col_name);
                enriched_rows[row_idx].values.push(related_data);
            }
        }

        Ok(enriched_rows)
    }

    /// Fetch related data for a single row based on a FETCH clause
    fn fetch_related_data(
        &self,
        source_row: &Row,
        fetch: &FetchClause,
        params: &QueryParams,
    ) -> Result<Value> {
        // First, try to find a defined relationship
        let relationship = self.relationships.get(&fetch.relationship);

        let (target_table_name, source_column, target_column) = if let Some(rel) = relationship {
            (
                rel.to_table.clone(),
                rel.from_column.clone(),
                rel.to_column.clone(),
            )
        } else {
            // Assume relationship name is the table name, use convention:
            // For fetching "orders" from a user row, assume user.id -> orders.user_id
            // Get the primary table's assumed id column name
            let target_table = fetch.relationship.clone();
            let source_col = "id".to_string();
            // Convention: target_table has a foreign key named <source_table>_id
            // But we don't know source table name here, so try common patterns
            let target_col = format!(
                "{}_id",
                source_row
                    .columns
                    .first()
                    .map(|_| "parent")
                    .unwrap_or("parent")
            );
            (target_table, source_col, target_col)
        };

        // Get the target table
        let target_table = match self.tables.get(&target_table_name) {
            Some(t) => t,
            None => {
                // Table not found, return empty array
                return Ok(Value::Text("[]".to_string()));
            }
        };

        // Get the source value to match against
        let source_idx = source_row.columns.iter().position(|c| c == &source_column);
        let source_value = match source_idx {
            Some(idx) => &source_row.values[idx],
            None => {
                // Try finding by id if not found
                source_row
                    .columns
                    .iter()
                    .position(|c| c == "id")
                    .map(|idx| &source_row.values[idx])
                    .unwrap_or(&Value::Null)
            }
        };

        // Find matching rows in the target table
        let target_col_idx = target_table
            .columns
            .iter()
            .position(|c| c == &target_column);

        let mut matching_rows: Vec<Row> = target_table
            .rows
            .iter()
            .filter(|row| {
                if let Some(idx) = target_col_idx {
                    &row.values[idx] == source_value
                } else {
                    // Try to match on first column as fallback
                    row.values.first() == Some(source_value)
                }
            })
            .cloned()
            .collect();

        // Apply WHERE clause to matching rows
        if let Some(where_expr) = &fetch.where_clause {
            matching_rows.retain(|row| evaluate_where(row, where_expr, params));
        }

        // Apply ORDER BY
        if let Some(order) = &fetch.order_by {
            matching_rows.sort_by(|a, b| {
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
        if let Some(lim) = fetch.limit {
            matching_rows.truncate(lim);
        }

        // Project columns
        let projected_rows = self.project_columns(&matching_rows, &fetch.columns, params)?;

        // Handle nested FETCH clauses
        let final_rows = if !fetch.nested_fetch.is_empty() {
            self.execute_fetch_clauses(
                &projected_rows,
                &matching_rows,
                &fetch.nested_fetch,
                params,
            )?
        } else {
            projected_rows
        };

        // Convert to JSON-like value (Text representation)
        let json = self.rows_to_json(&final_rows);
        Ok(Value::Text(json))
    }

    /// Convert rows to a JSON array string
    fn rows_to_json(&self, rows: &[Row]) -> String {
        let json_objects: Vec<String> = rows
            .iter()
            .map(|row| {
                let fields: Vec<String> = row
                    .columns
                    .iter()
                    .zip(row.values.iter())
                    .map(|(col, val)| {
                        let json_val = match val {
                            Value::Null => "null".to_string(),
                            Value::Integer(i) => i.to_string(),
                            Value::Float(f) => f.to_string(),
                            Value::Text(s) => format!("\"{}\"", s.replace('"', "\\\"")),
                            Value::Boolean(b) => b.to_string(),
                            Value::Blob(b) => format!("\"<blob:{} bytes>\"", b.len()),
                            Value::Vector(v) => format!(
                                "[{}]",
                                v.iter()
                                    .map(|f| f.to_string())
                                    .collect::<Vec<_>>()
                                    .join(",")
                            ),
                            Value::Geometry(g) => serde_json::to_string(&g.as_text()).unwrap(),
                        };
                        format!("\"{}\":{}", col, json_val)
                    })
                    .collect();
                format!("{{{}}}", fields.join(","))
            })
            .collect();
        format!("[{}]", json_objects.join(","))
    }

    /// Execute a TRAVERSE statement for graph operations
    fn execute_traverse(
        &self,
        clause: &TraverseClause,
        from_table: &str,
        where_clause: &Option<Expr>,
        return_columns: &[SelectItem],
        limit: &Option<usize>,
        params: &QueryParams,
    ) -> Result<ResultSet> {
        // Get the starting table
        let start_table = self.tables.get(from_table).ok_or_else(|| {
            crate::NexusError::QueryError(format!("Table {} not found", from_table))
        })?;

        // Get starting nodes that match the WHERE clause
        let mut start_nodes: Vec<Row> = start_table.rows.clone();

        if let Some(start_where) = &clause.start_where {
            start_nodes.retain(|row| evaluate_where(row, start_where, params));
        }

        if let Some(filter_where) = where_clause {
            start_nodes.retain(|row| evaluate_where(row, filter_where, params));
        }

        // Perform traversal based on mode
        let traversed_rows = match clause.mode {
            TraverseMode::All => self.traverse_all(&start_nodes, clause, params)?,
            TraverseMode::Bfs => self.traverse_bfs(&start_nodes, clause, params)?,
            TraverseMode::Dfs => self.traverse_dfs(&start_nodes, clause, params)?,
            TraverseMode::ShortestPath => {
                // Shortest path requires a target - for now, return BFS results
                self.traverse_bfs(&start_nodes, clause, params)?
            }
            TraverseMode::Cycle => self.detect_cycles(&start_nodes, clause, params)?,
        };

        // Apply LIMIT
        let mut result_rows = traversed_rows;
        if let Some(lim) = limit {
            result_rows.truncate(*lim);
        }

        // Project return columns
        let projected = self.project_columns(&result_rows, return_columns, params)?;

        Ok(ResultSet::new(projected))
    }

    /// Traverse all paths using relationship pattern
    fn traverse_all(
        &self,
        start_nodes: &[Row],
        clause: &TraverseClause,
        params: &QueryParams,
    ) -> Result<Vec<Row>> {
        let mut results = Vec::new();
        let relationship_type = &clause.pattern.relationship_type;

        // Find relationship or use convention
        let rel = self.relationships.get(relationship_type);

        for start_node in start_nodes {
            let paths = self.find_paths(start_node, clause, rel, params, 0, &mut vec![]);
            results.extend(paths);
        }

        Ok(results)
    }

    /// Breadth-first search traversal
    fn traverse_bfs(
        &self,
        start_nodes: &[Row],
        clause: &TraverseClause,
        params: &QueryParams,
    ) -> Result<Vec<Row>> {
        use std::collections::VecDeque;

        let mut results = Vec::new();
        let mut visited: std::collections::HashSet<Vec<Value>> = std::collections::HashSet::new();
        let mut queue: VecDeque<(Row, usize)> = VecDeque::new();

        let rel = self.relationships.get(&clause.pattern.relationship_type);

        for start_node in start_nodes {
            queue.push_back((start_node.clone(), 0));
            visited.insert(start_node.values.clone());
        }

        while let Some((current, depth)) = queue.pop_front() {
            if depth >= clause.min_depth {
                results.push(current.clone());
            }

            if depth < clause.max_depth {
                let neighbors = self.get_neighbors(&current, rel, clause, params)?;
                for neighbor in neighbors {
                    if !visited.contains(&neighbor.values) {
                        visited.insert(neighbor.values.clone());
                        queue.push_back((neighbor, depth + 1));
                    }
                }
            }
        }

        Ok(results)
    }

    /// Depth-first search traversal
    fn traverse_dfs(
        &self,
        start_nodes: &[Row],
        clause: &TraverseClause,
        params: &QueryParams,
    ) -> Result<Vec<Row>> {
        let mut results = Vec::new();
        let mut visited: std::collections::HashSet<Vec<Value>> = std::collections::HashSet::new();

        let rel = self.relationships.get(&clause.pattern.relationship_type);

        for start_node in start_nodes {
            self.dfs_visit(
                start_node,
                0,
                clause,
                rel,
                params,
                &mut visited,
                &mut results,
            )?;
        }

        Ok(results)
    }

    /// DFS helper function
    fn dfs_visit(
        &self,
        node: &Row,
        depth: usize,
        clause: &TraverseClause,
        rel: Option<&Relationship>,
        params: &QueryParams,
        visited: &mut std::collections::HashSet<Vec<Value>>,
        results: &mut Vec<Row>,
    ) -> Result<()> {
        if visited.contains(&node.values) {
            return Ok(());
        }
        visited.insert(node.values.clone());

        if depth >= clause.min_depth {
            results.push(node.clone());
        }

        if depth < clause.max_depth {
            let neighbors = self.get_neighbors(node, rel, clause, params)?;
            for neighbor in neighbors {
                self.dfs_visit(&neighbor, depth + 1, clause, rel, params, visited, results)?;
            }
        }

        Ok(())
    }

    /// Detect cycles in the graph
    fn detect_cycles(
        &self,
        start_nodes: &[Row],
        clause: &TraverseClause,
        params: &QueryParams,
    ) -> Result<Vec<Row>> {
        let mut cycles = Vec::new();
        let rel = self.relationships.get(&clause.pattern.relationship_type);

        for start_node in start_nodes {
            let mut path: Vec<Vec<Value>> = vec![start_node.values.clone()];
            let mut visiting: std::collections::HashSet<Vec<Value>> =
                std::collections::HashSet::new();
            visiting.insert(start_node.values.clone());

            self.find_cycles(
                start_node,
                0,
                clause,
                rel,
                params,
                &mut path,
                &mut visiting,
                &mut cycles,
            )?;
        }

        Ok(cycles)
    }

    /// Find cycles helper
    fn find_cycles(
        &self,
        node: &Row,
        depth: usize,
        clause: &TraverseClause,
        rel: Option<&Relationship>,
        params: &QueryParams,
        path: &mut Vec<Vec<Value>>,
        visiting: &mut std::collections::HashSet<Vec<Value>>,
        cycles: &mut Vec<Row>,
    ) -> Result<()> {
        if depth >= clause.max_depth {
            return Ok(());
        }

        let neighbors = self.get_neighbors(node, rel, clause, params)?;

        for neighbor in neighbors {
            if visiting.contains(&neighbor.values) {
                // Found a cycle
                cycles.push(neighbor.clone());
            } else {
                path.push(neighbor.values.clone());
                visiting.insert(neighbor.values.clone());

                self.find_cycles(
                    &neighbor,
                    depth + 1,
                    clause,
                    rel,
                    params,
                    path,
                    visiting,
                    cycles,
                )?;

                path.pop();
                visiting.remove(&neighbor.values);
            }
        }

        Ok(())
    }

    /// Find all paths from a starting node
    fn find_paths(
        &self,
        node: &Row,
        clause: &TraverseClause,
        rel: Option<&Relationship>,
        params: &QueryParams,
        depth: usize,
        visited: &mut Vec<Vec<Value>>,
    ) -> Vec<Row> {
        let mut results = Vec::new();

        if depth >= clause.min_depth {
            results.push(node.clone());
        }

        if depth >= clause.max_depth {
            return results;
        }

        if visited.contains(&node.values) {
            return results;
        }

        visited.push(node.values.clone());

        if let Ok(neighbors) = self.get_neighbors(node, rel, clause, params) {
            for neighbor in neighbors {
                let sub_paths = self.find_paths(&neighbor, clause, rel, params, depth + 1, visited);
                results.extend(sub_paths);
            }
        }

        visited.pop();

        results
    }

    /// Get neighbors of a node based on relationship
    fn get_neighbors(
        &self,
        node: &Row,
        rel: Option<&Relationship>,
        clause: &TraverseClause,
        _params: &QueryParams,
    ) -> Result<Vec<Row>> {
        let (target_table_name, source_col, target_col) = if let Some(r) = rel {
            match clause.pattern.direction {
                TraverseDirection::Outgoing => (
                    r.to_table.clone(),
                    r.from_column.clone(),
                    r.to_column.clone(),
                ),
                TraverseDirection::Incoming => (
                    r.from_table.clone(),
                    r.to_column.clone(),
                    r.from_column.clone(),
                ),
                TraverseDirection::Both => {
                    // For bidirectional, we'd need to check both directions
                    // For now, default to outgoing
                    (
                        r.to_table.clone(),
                        r.from_column.clone(),
                        r.to_column.clone(),
                    )
                }
            }
        } else {
            // Use convention: relationship_type is the target table
            let table_name = clause.pattern.relationship_type.clone();
            (
                "".to_string(),
                "id".to_string(),
                format!("{}_id", table_name),
            )
        };

        // If no relationship defined and no table found, return empty
        if target_table_name.is_empty() {
            return Ok(Vec::new());
        }

        let target_table = match self.tables.get(&target_table_name) {
            Some(t) => t,
            None => return Ok(Vec::new()),
        };

        // Get source value
        let source_idx = node.columns.iter().position(|c| c == &source_col);
        let source_value = match source_idx {
            Some(idx) => &node.values[idx],
            None => return Ok(Vec::new()),
        };

        // Find target column index
        let target_col_idx = target_table.columns.iter().position(|c| c == &target_col);

        let neighbors: Vec<Row> = target_table
            .rows
            .iter()
            .filter(|row| {
                if let Some(idx) = target_col_idx {
                    &row.values[idx] == source_value
                } else {
                    false
                }
            })
            .cloned()
            .collect();

        Ok(neighbors)
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

#[cfg(test)]
#[path = "executor_set_operations_tests.rs"]
mod executor_set_operations_tests;

#[cfg(test)]
#[path = "executor_fetch_traverse_tests.rs"]
mod executor_fetch_traverse_tests;

#[cfg(test)]
#[path = "executor_join_tests.rs"]
mod executor_join_tests;

#[cfg(test)]
#[path = "executor_group_by_tests.rs"]
mod executor_group_by_tests;

#[cfg(test)]
#[path = "executor_advanced_tests.rs"]
mod executor_advanced_tests;

#[cfg(test)]
#[path = "executor_subquery_tests.rs"]
mod executor_subquery_tests;

#[cfg(test)]
#[path = "executor_window_tests.rs"]
mod executor_window_tests;
