//! SQL Execution Engine
//! PROPRIETARY

use crate::query::ast::*;
use crate::query::evaluator::{evaluate_expr, evaluate_where};
use crate::storage::Table;
use crate::types::{ResultSet, Row, Value};
use crate::Result;
use std::collections::HashMap;

pub struct Executor {
    tables: HashMap<String, Table>,
}

impl Executor {
    pub fn new() -> Self {
        Self {
            tables: HashMap::new(),
        }
    }

    pub fn execute(&mut self, statement: &Statement) -> Result<ResultSet> {
        match statement {
            Statement::CreateTable { name, columns } => {
                let col_names: Vec<String> = columns.iter().map(|c| c.name.clone()).collect();
                let table = Table::new(name.to_string(), col_names);
                self.tables.insert(name.to_string(), table);
                Ok(ResultSet::empty())
            }
            Statement::Insert {
                table,
                columns: _,
                values,
            } => {
                if let Some(t) = self.tables.get_mut(table) {
                    for value_set in values {
                        let row = Row::new(t.columns.clone(), value_set.clone());
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
                    rows.retain(|row| evaluate_where(row, expr));
                }

                // Apply ORDER BY sorting (especially for vector distance)
                if let Some(order) = order_by {
                    rows.sort_by(|a, b| {
                        use crate::query::evaluator::evaluate_expr;
                        let a_val = evaluate_expr(a, &order.expr);
                        let b_val = evaluate_expr(b, &order.expr);

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
                let result_rows = self.project_columns(&rows, columns)?;

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
                            .map(|expr| evaluate_where(row, expr))
                            .unwrap_or(true);

                        if should_update {
                            for (col_name, new_value) in set {
                                if let Some(idx) = row.columns.iter().position(|c| c == col_name) {
                                    row.values[idx] = new_value.clone();
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
                        t.rows.retain(|row| !evaluate_where(row, expr));
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
        }
    }

    pub fn get_tables(&self) -> &HashMap<String, Table> {
        &self.tables
    }

    pub fn get_tables_mut(&mut self) -> &mut HashMap<String, Table> {
        &mut self.tables
    }

    /// Project columns from rows based on SELECT list items
    fn project_columns(&self, rows: &[Row], select_items: &[SelectItem]) -> Result<Vec<Row>> {
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
                        let value = evaluate_expr(row, expr);

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
