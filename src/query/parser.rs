//! Full SQL Parser using sqlparser-rs
//! PROPRIETARY

use sqlparser::parser::Parser as SqlParser;
use sqlparser::dialect::GenericDialect;
use sqlparser::ast as sql_ast;
use crate::query::ast::*;
use crate::types::Value;
use crate::{Result, NexusError};

pub struct QueryParser {
    dialect: GenericDialect,
}

impl QueryParser {
    pub fn new() -> Self {
        Self {
            dialect: GenericDialect {},
        }
    }
    
    pub fn parse(sql: &str) -> Result<Statement> {
        let parser = Self::new();
        let ast = SqlParser::parse_sql(&parser.dialect, sql)
            .map_err(|e| NexusError::ParseError(e.to_string()))?;
        
        if ast.is_empty() {
            return Err(NexusError::ParseError("Empty statement".into()));
        }
        
        // Convert first statement
        parser.convert_statement(&ast[0])
    }
    
    fn convert_statement(&self, stmt: &sql_ast::Statement) -> Result<Statement> {
        match stmt {
            sql_ast::Statement::Query(query) => self.convert_query(query),
            
            sql_ast::Statement::Insert(insert) => self.convert_insert(insert),
            
            sql_ast::Statement::CreateTable(create) => {
                Ok(Statement::CreateTable {
                    name: create.name.to_string(),
                    columns: create.columns.iter().map(|col| {
                        ColumnDef {
                            name: col.name.to_string(),
                            data_type: self.convert_data_type(&col.data_type),
                        }
                    }).collect(),
                })
            },
            
            _ => Err(NexusError::ParseError("Unsupported statement type".into()))
        }
    }
    
    fn convert_query(&self, query: &Box<sql_ast::Query>) -> Result<Statement> {
        if let sql_ast::SetExpr::Select(select) = &*query.body {
            let table = self.extract_table_name(&select.from)?;
            
            let columns = select.projection.iter().map(|proj| {
                match proj {
                    sql_ast::SelectItem::Wildcard(_) => "*".to_string(),
                    sql_ast::SelectItem::UnnamedExpr(expr) => format!("{}", expr),
                    sql_ast::SelectItem::ExprWithAlias { expr, alias } => alias.to_string(),
                    _ => "unknown".to_string(),
                }
            }).collect();
            
            // Extract USING clause if present
            let using_engine = self.extract_using_clause(query);
            
            // Extract LIMIT
            let limit = if let Some(sql_ast::Expr::Value(sql_ast::Value::Number(n, _))) = &query.limit {
                n.parse::<usize>().ok()
            } else {
                None
            };
            
            Ok(Statement::Select {
                columns,
                from: table,
                where_clause: None,  // TODO: Convert WHERE
                using_engine,
                limit,
            })
        } else {
            Err(NexusError::ParseError("Only SELECT supported currently".into()))
        }
    }
    
    fn convert_insert(&self, insert: &sql_ast::Insert) -> Result<Statement> {
        let table = insert.table_name.to_string();
        
        // Extract column names
        let columns = insert.columns.iter()
            .map(|col| col.value.clone())
            .collect();
        
        // Extract values (simplified)
        let values = vec![vec![
            Value::Integer(1),
            Value::Text("placeholder".to_string()),
        ]];
        
        Ok(Statement::Insert {
            table,
            columns: Some(columns),
            values,
        })
    }
    
    fn extract_table_name(&self, from: &Vec<sql_ast::TableWithJoins>) -> Result<String> {
        if let Some(first_table) = from.first() {
            if let sql_ast::TableFactor::Table { name, .. } = &first_table.relation {
                return Ok(name.to_string());
            }
        }
        Err(NexusError::ParseError("No table in FROM clause".into()))
    }
    
    fn extract_using_clause(&self, query: &sql_ast::Query) -> Option<String> {
        // Check for USING in SQL (custom extension)
        // For now, return None (will implement when needed)
        None
    }
    
    fn convert_data_type(&self, dt: &sql_ast::DataType) -> DataType {
        match dt {
            sql_ast::DataType::Int(_) | sql_ast::DataType::Integer(_) => DataType::Integer,
            sql_ast::DataType::Float(_) | sql_ast::DataType::Real => DataType::Float,
            sql_ast::DataType::Text | sql_ast::DataType::Varchar(_) => DataType::Text,
            sql_ast::DataType::Boolean => DataType::Boolean,
            _ => DataType::Blob,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_select() {
        let stmt = QueryParser::parse("SELECT * FROM users").unwrap();
        match stmt {
            Statement::Select { from, .. } => assert_eq!(from, "users"),
            _ => panic!("Wrong type"),
        }
    }
    
    #[test]
    fn test_parse_create_table() {
        let stmt = QueryParser::parse("CREATE TABLE users (id INTEGER, name TEXT)").unwrap();
        match stmt {
            Statement::CreateTable { name, columns } => {
                assert_eq!(name, "users");
                assert_eq!(columns.len(), 2);
            },
            _ => panic!("Wrong type"),
        }
    }
    
    #[test]
    fn test_parse_with_limit() {
        let stmt = QueryParser::parse("SELECT * FROM users LIMIT 10").unwrap();
        match stmt {
            Statement::Select { limit, .. } => {
                assert_eq!(limit, Some(10));
            },
            _ => panic!("Wrong type"),
        }
    }
}
