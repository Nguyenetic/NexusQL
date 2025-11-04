//! Full SQL Parser using sqlparser-rs
//! PROPRIETARY

use crate::query::ast::*;
use crate::types::Value;
use crate::{NexusError, Result};
use sqlparser::ast as sql_ast;
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser as SqlParser;

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

            sql_ast::Statement::CreateTable(create) => Ok(Statement::CreateTable {
                name: create.name.to_string(),
                columns: create
                    .columns
                    .iter()
                    .map(|col| ColumnDef {
                        name: col.name.to_string(),
                        data_type: self.convert_data_type(&col.data_type),
                    })
                    .collect(),
            }),

            sql_ast::Statement::Commit { .. } => {
                // Handle COMMIT - just return empty result
                Ok(Statement::Select {
                    columns: vec![],
                    from: String::new(),
                    where_clause: None,
                    using_engine: None,
                    order_by: None,
                    limit: None,
                })
            }

            sql_ast::Statement::Rollback { .. } => {
                // Handle ROLLBACK - just return empty result
                Ok(Statement::Select {
                    columns: vec![],
                    from: String::new(),
                    where_clause: None,
                    using_engine: None,
                    order_by: None,
                    limit: None,
                })
            }

            sql_ast::Statement::Update {
                table,
                assignments,
                selection,
                ..
            } => {
                let table_name = table.to_string();

                // Convert SET assignments
                let mut set_map = Vec::new();
                for assignment in assignments {
                    let col_name = match &assignment.target {
                        sql_ast::AssignmentTarget::ColumnName(obj_name) => obj_name
                            .0
                            .iter()
                            .map(|id| id.value.clone())
                            .collect::<Vec<_>>()
                            .join("."),
                        _ => {
                            return Err(NexusError::ParseError(
                                "Unsupported assignment target".into(),
                            ))
                        }
                    };
                    let value = self.convert_value_expr(&assignment.value)?;
                    set_map.push((col_name, value));
                }

                // Convert WHERE clause (if any)
                let where_clause = selection
                    .as_ref()
                    .map(|expr| self.convert_where_expr(expr))
                    .transpose()?;

                Ok(Statement::Update {
                    table: table_name,
                    set: set_map,
                    where_clause,
                })
            }

            sql_ast::Statement::Delete(delete) => {
                // Extract table name from DELETE statement
                let table_name = match &delete.from {
                    sql_ast::FromTable::WithFromKeyword(tables) => {
                        if let Some(table_with_joins) = tables.first() {
                            if let sql_ast::TableFactor::Table { name, .. } =
                                &table_with_joins.relation
                            {
                                name.to_string()
                            } else {
                                return Err(NexusError::ParseError(
                                    "Unsupported DELETE FROM structure".into(),
                                ));
                            }
                        } else {
                            return Err(NexusError::ParseError(
                                "DELETE without FROM clause".into(),
                            ));
                        }
                    }
                    sql_ast::FromTable::WithoutKeyword(tables) => {
                        if let Some(from) = tables.first() {
                            from.to_string()
                        } else {
                            return Err(NexusError::ParseError("DELETE without table".into()));
                        }
                    }
                };

                // Convert WHERE clause (if any)
                let where_clause = delete
                    .selection
                    .as_ref()
                    .map(|expr| self.convert_where_expr(expr))
                    .transpose()?;

                Ok(Statement::Delete {
                    from: table_name,
                    where_clause,
                })
            }

            _ => Err(NexusError::ParseError("Unsupported statement type".into())),
        }
    }

    fn convert_query(&self, query: &Box<sql_ast::Query>) -> Result<Statement> {
        if let sql_ast::SetExpr::Select(select) = &*query.body {
            let table = self.extract_table_name(&select.from)?;

            let columns = select
                .projection
                .iter()
                .map(|proj| match proj {
                    sql_ast::SelectItem::Wildcard(_) => "*".to_string(),
                    sql_ast::SelectItem::UnnamedExpr(expr) => format!("{}", expr),
                    sql_ast::SelectItem::ExprWithAlias { expr: _, alias } => alias.to_string(),
                    _ => "unknown".to_string(),
                })
                .collect();

            // Extract USING clause if present
            let using_engine = self.extract_using_clause(query);

            // Extract ORDER BY
            let order_by = if let Some(order_by_clause) = &query.order_by {
                if !order_by_clause.exprs.is_empty() {
                    let order_item = &order_by_clause.exprs[0];
                    let expr = self.convert_where_expr(&order_item.expr)?;
                    let ascending = order_item.asc.unwrap_or(true);
                    Some(OrderBy { expr, ascending })
                } else {
                    None
                }
            } else {
                None
            };

            // Extract LIMIT
            let limit =
                if let Some(sql_ast::Expr::Value(sql_ast::Value::Number(n, _))) = &query.limit {
                    n.parse::<usize>().ok()
                } else {
                    None
                };

            Ok(Statement::Select {
                columns,
                from: table,
                where_clause: None, // TODO: Convert WHERE
                using_engine,
                order_by,
                limit,
            })
        } else {
            Err(NexusError::ParseError(
                "Only SELECT supported currently".into(),
            ))
        }
    }

    fn convert_insert(&self, insert: &sql_ast::Insert) -> Result<Statement> {
        let table = insert.table_name.to_string();

        // Extract column names
        let columns = if !insert.columns.is_empty() {
            Some(insert.columns.iter().map(|col| col.value.clone()).collect())
        } else {
            None
        };

        // Extract values from the source
        let mut values = Vec::new();

        if let Some(query_box) = &insert.source {
            if let sql_ast::SetExpr::Values(vals) = &*query_box.body {
                for row in &vals.rows {
                    let mut row_values = Vec::new();
                    for expr in row {
                        row_values.push(self.convert_value_expr(expr)?);
                    }
                    values.push(row_values);
                }
            }
        }

        Ok(Statement::Insert {
            table,
            columns,
            values,
        })
    }

    fn convert_value_expr(&self, expr: &sql_ast::Expr) -> Result<Value> {
        match expr {
            sql_ast::Expr::Value(val) => match val {
                sql_ast::Value::Number(n, _) => {
                    if let Ok(i) = n.parse::<i64>() {
                        Ok(Value::Integer(i))
                    } else if let Ok(f) = n.parse::<f64>() {
                        Ok(Value::Float(f.into()))
                    } else {
                        Err(NexusError::ParseError(format!("Invalid number: {}", n)))
                    }
                }
                sql_ast::Value::SingleQuotedString(s) | sql_ast::Value::DoubleQuotedString(s) => {
                    // Check if this is a vector literal like '[0.1, 0.2, 0.3]'
                    if s.starts_with('[') && s.ends_with(']') {
                        return self.parse_vector_literal(s);
                    }
                    Ok(Value::Text(s.clone()))
                }
                sql_ast::Value::Boolean(b) => Ok(Value::Boolean(*b)),
                sql_ast::Value::Null => Ok(Value::Null),
                _ => Err(NexusError::ParseError("Unsupported value type".into())),
            },
            // Handle array syntax: ARRAY[0.1, 0.2, 0.3] or [0.1, 0.2, 0.3]
            sql_ast::Expr::Array(sql_ast::Array { elem, .. }) => {
                let mut vector = Vec::new();
                for element in elem {
                    if let sql_ast::Expr::Value(sql_ast::Value::Number(n, _)) = element {
                        if let Ok(f) = n.parse::<f32>() {
                            vector.push(f);
                        } else {
                            return Err(NexusError::ParseError(format!(
                                "Invalid vector element: {}",
                                n
                            )));
                        }
                    } else {
                        return Err(NexusError::ParseError(
                            "Vector elements must be numbers".into(),
                        ));
                    }
                }
                Ok(Value::Vector(vector))
            }
            _ => Err(NexusError::ParseError(
                "Complex expressions not yet supported in INSERT".into(),
            )),
        }
    }

    /// Parse vector literal from string like '[0.1, 0.2, 0.3]'
    fn parse_vector_literal(&self, s: &str) -> Result<Value> {
        let trimmed = s.trim_start_matches('[').trim_end_matches(']');
        let mut vector = Vec::new();

        for part in trimmed.split(',') {
            let part = part.trim();
            if let Ok(f) = part.parse::<f32>() {
                vector.push(f);
            } else {
                return Err(NexusError::ParseError(format!(
                    "Invalid vector element: {}",
                    part
                )));
            }
        }

        Ok(Value::Vector(vector))
    }

    fn extract_table_name(&self, from: &Vec<sql_ast::TableWithJoins>) -> Result<String> {
        if let Some(first_table) = from.first() {
            if let sql_ast::TableFactor::Table { name, .. } = &first_table.relation {
                return Ok(name.to_string());
            }
        }
        Err(NexusError::ParseError("No table in FROM clause".into()))
    }

    fn extract_using_clause(&self, _query: &sql_ast::Query) -> Option<String> {
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
            // VECTOR(n) data type for embeddings
            sql_ast::DataType::Custom(obj_name, args) => {
                if obj_name.to_string().to_uppercase() == "VECTOR" && !args.is_empty() {
                    // Extract dimension from VECTOR(384)
                    // args is Vec<String>, so parse the first arg
                    if let Ok(dim) = args[0].parse::<usize>() {
                        return DataType::Vector(dim);
                    }
                }
                DataType::Blob // Fallback for unknown custom types
            }
            _ => DataType::Blob,
        }
    }

    fn convert_where_expr(&self, expr: &sql_ast::Expr) -> Result<Expr> {
        match expr {
            sql_ast::Expr::BinaryOp { left, op, right } => {
                let left_expr = self.convert_where_expr(left)?;
                let right_expr = self.convert_where_expr(right)?;

                let binary_op = match op {
                    sql_ast::BinaryOperator::Eq => BinaryOperator::Eq,
                    sql_ast::BinaryOperator::NotEq => BinaryOperator::NotEq,
                    sql_ast::BinaryOperator::Lt => BinaryOperator::Lt,
                    sql_ast::BinaryOperator::LtEq => BinaryOperator::LtEq,
                    sql_ast::BinaryOperator::Gt => BinaryOperator::Gt,
                    sql_ast::BinaryOperator::GtEq => BinaryOperator::GtEq,
                    sql_ast::BinaryOperator::And => BinaryOperator::And,
                    sql_ast::BinaryOperator::Or => BinaryOperator::Or,
                    // Vector distance operators (pgvector-compatible)
                    sql_ast::BinaryOperator::Custom(op_str) => match op_str.as_str() {
                        "<->" => BinaryOperator::L2Distance,
                        "<=>" => BinaryOperator::CosineDistance,
                        "<#>" => BinaryOperator::InnerProduct,
                        _ => {
                            return Err(NexusError::ParseError(format!(
                                "Unsupported custom operator: {}",
                                op_str
                            )))
                        }
                    },
                    _ => {
                        return Err(NexusError::ParseError(format!(
                            "Unsupported operator: {:?}",
                            op
                        )))
                    }
                };

                Ok(Expr::BinaryOp {
                    left: Box::new(left_expr),
                    op: binary_op,
                    right: Box::new(right_expr),
                })
            }

            sql_ast::Expr::Identifier(ident) => Ok(Expr::Column(ident.value.clone())),

            sql_ast::Expr::Value(val) => {
                let value = match val {
                    sql_ast::Value::Number(n, _) => {
                        if let Ok(i) = n.parse::<i64>() {
                            Value::Integer(i)
                        } else if let Ok(f) = n.parse::<f64>() {
                            Value::Float(f.into())
                        } else {
                            return Err(NexusError::ParseError(format!("Invalid number: {}", n)));
                        }
                    }
                    sql_ast::Value::SingleQuotedString(s)
                    | sql_ast::Value::DoubleQuotedString(s) => {
                        // Check if this is a vector literal
                        if s.starts_with('[') && s.ends_with(']') {
                            self.parse_vector_literal(s)?
                        } else {
                            Value::Text(s.clone())
                        }
                    }
                    sql_ast::Value::Boolean(b) => Value::Boolean(*b),
                    sql_ast::Value::Null => Value::Null,
                    _ => {
                        return Err(NexusError::ParseError(
                            "Unsupported value type in WHERE".into(),
                        ))
                    }
                };
                Ok(Expr::Literal(value))
            }

            // Handle array syntax in WHERE clause
            sql_ast::Expr::Array(sql_ast::Array { elem, .. }) => {
                let mut vector = Vec::new();
                for element in elem {
                    if let sql_ast::Expr::Value(sql_ast::Value::Number(n, _)) = element {
                        if let Ok(f) = n.parse::<f32>() {
                            vector.push(f);
                        } else {
                            return Err(NexusError::ParseError(format!(
                                "Invalid vector element: {}",
                                n
                            )));
                        }
                    } else {
                        return Err(NexusError::ParseError(
                            "Vector elements must be numbers".into(),
                        ));
                    }
                }
                Ok(Expr::Literal(Value::Vector(vector)))
            }

            _ => Err(NexusError::ParseError(
                "Complex WHERE expressions not yet supported".into(),
            )),
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
            }
            _ => panic!("Wrong type"),
        }
    }

    #[test]
    fn test_parse_with_limit() {
        let stmt = QueryParser::parse("SELECT * FROM users LIMIT 10").unwrap();
        match stmt {
            Statement::Select { limit, .. } => {
                assert_eq!(limit, Some(10));
            }
            _ => panic!("Wrong type"),
        }
    }
}
