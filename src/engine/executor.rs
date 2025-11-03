//! SQL Execution Engine
//! PROPRIETARY

use crate::query::ast::*;
use crate::query::evaluator::evaluate_where;
use crate::types::{Row, ResultSet};
use crate::storage::Table;
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
            },
            Statement::Insert { table, columns: _, values } => {
                if let Some(t) = self.tables.get_mut(table) {
                    for value_set in values {
                        let row = Row::new(t.columns.clone(), value_set.clone());
                        t.insert(row);
                    }
                    Ok(ResultSet::empty())
                } else {
                    Err(crate::NexusError::QueryError(format!("Table {} not found", table)))
                }
            },
            Statement::Select { columns, from, where_clause, using_engine: _, limit } => {
                let table = self.tables.get(from)
                    .ok_or_else(|| crate::NexusError::QueryError(format!("Table {} not found", from)))?;
                
                let mut rows = table.select_all();
                
                if let Some(expr) = where_clause {
                    rows.retain(|row| evaluate_where(row, expr));
                }
                
                if let Some(lim) = limit {
                    rows.truncate(*lim);
                }
                
                Ok(ResultSet::new(rows))
            },
            Statement::Update { table, set, where_clause } => {
                if let Some(t) = self.tables.get_mut(table) {
                    for row in &mut t.rows {
                        let should_update = where_clause.as_ref()
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
                    Err(crate::NexusError::QueryError(format!("Table {} not found", table)))
                }
            },
            Statement::Delete { from, where_clause } => {
                if let Some(t) = self.tables.get_mut(from) {
                    if let Some(expr) = where_clause {
                        t.rows.retain(|row| !evaluate_where(row, expr));
                    } else {
                        t.rows.clear();
                    }
                    Ok(ResultSet::empty())
                } else {
                    Err(crate::NexusError::QueryError(format!("Table {} not found", from)))
                }
            },
        }
    }
    
    pub fn get_tables(&self) -> &HashMap<String, Table> {
        &self.tables
    }
    
    pub fn get_tables_mut(&mut self) -> &mut HashMap<String, Table> {
        &mut self.tables
    }
}
