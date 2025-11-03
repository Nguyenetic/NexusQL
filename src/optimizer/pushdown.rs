//! Pushdown Optimizations
//! PROPRIETARY

use crate::query::ast::*;

pub struct PushdownAnalyzer;

impl PushdownAnalyzer {
    pub fn extract_pushable_predicates(where_clause: &Expr, table: &str) -> Vec<Expr> {
        let mut predicates = Vec::new();
        Self::collect_predicates(where_clause, &mut predicates);
        predicates
            .into_iter()
            .filter(|pred| Self::references_only_table(pred, table))
            .collect()
    }

    fn collect_predicates(expr: &Expr, predicates: &mut Vec<Expr>) {
        match expr {
            Expr::BinaryOp {
                left,
                op: BinaryOperator::And,
                right,
            } => {
                Self::collect_predicates(left, predicates);
                Self::collect_predicates(right, predicates);
            }
            _ => {
                predicates.push(expr.clone());
            }
        }
    }

    fn references_only_table(_expr: &Expr, _table: &str) -> bool {
        true
    }

    pub fn extract_needed_columns(statement: &Statement) -> Vec<String> {
        match statement {
            Statement::Select {
                columns,
                where_clause,
                ..
            } => {
                let mut needed = columns.clone();
                if let Some(expr) = where_clause {
                    let where_cols = Self::extract_columns_from_expr(expr);
                    needed.extend(where_cols);
                }
                needed.sort();
                needed.dedup();
                needed
            }
            _ => vec![],
        }
    }

    fn extract_columns_from_expr(expr: &Expr) -> Vec<String> {
        match expr {
            Expr::Column(name) => vec![name.clone()],
            Expr::BinaryOp { left, right, .. } => {
                let mut cols = Self::extract_columns_from_expr(left);
                cols.extend(Self::extract_columns_from_expr(right));
                cols
            }
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_columns() {
        let statement = Statement::Select {
            columns: vec!["name".to_string(), "age".to_string()],
            from: "users".to_string(),
            where_clause: Some(Expr::BinaryOp {
                left: Box::new(Expr::Column("age".to_string())),
                op: BinaryOperator::Gt,
                right: Box::new(Expr::Literal(Value::Integer(18))),
            }),
            using_engine: None,
            limit: None,
        };

        let cols = PushdownAnalyzer::extract_needed_columns(&statement);

        assert!(cols.contains(&"name".to_string()));
        assert!(cols.contains(&"age".to_string()));
    }
}
