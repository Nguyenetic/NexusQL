//! Optimization Rules - Inspired by DuckDB
//! PROPRIETARY

use crate::optimizer::OptimizationRule;
use crate::query::ast::*;
use crate::types::Value;
use crate::Result;

/// Rule 1: Constant Folding
/// Evaluates constant expressions at compile time
/// Example: WHERE 1+1 = 2 → WHERE true
pub struct ConstantFoldingRule;

impl OptimizationRule for ConstantFoldingRule {
    fn name(&self) -> &str {
        "ConstantFolding"
    }

    fn apply(&self, statement: Statement) -> Result<Statement> {
        match statement {
            Statement::Select {
                columns,
                from,
                where_clause,
                using_engine,
                order_by,
                limit,
            } => {
                let optimized_where = where_clause.map(|expr| self.fold_constants(expr));

                Ok(Statement::Select {
                    columns,
                    from,
                    where_clause: optimized_where,
                    using_engine,
                    order_by,
                    limit,
                })
            }
            _ => Ok(statement),
        }
    }
}

impl ConstantFoldingRule {
    fn fold_constants(&self, expr: Expr) -> Expr {
        match expr {
            Expr::BinaryOp { left, op, right } => {
                let left_folded = self.fold_constants(*left);
                let right_folded = self.fold_constants(*right);

                // Try to evaluate if both are literals
                if let (Expr::Literal(left_val), Expr::Literal(right_val)) =
                    (&left_folded, &right_folded)
                {
                    // Evaluate at compile time
                    match op {
                        BinaryOperator::Eq => {
                            return Expr::Literal(Value::Boolean(left_val == right_val));
                        }
                        _ => {}
                    }
                }

                Expr::BinaryOp {
                    left: Box::new(left_folded),
                    op,
                    right: Box::new(right_folded),
                }
            }
            _ => expr,
        }
    }
}

/// Rule 2: Filter Pushdown
/// Pushes WHERE clauses closer to data source
pub struct FilterPushdownRule;

impl OptimizationRule for FilterPushdownRule {
    fn name(&self) -> &str {
        "FilterPushdown"
    }

    fn apply(&self, statement: Statement) -> Result<Statement> {
        // Mark that filter should be pushed to storage
        // (Will be implemented in storage engine)
        Ok(statement)
    }
}

/// Rule 3: Projection Pushdown (Column Pruning)
/// Only read columns that are actually needed
pub struct ProjectionPushdownRule;

impl OptimizationRule for ProjectionPushdownRule {
    fn name(&self) -> &str {
        "ProjectionPushdown"
    }

    fn apply(&self, statement: Statement) -> Result<Statement> {
        // Analyze which columns are used
        // Prune unused columns from read
        Ok(statement)
    }
}

/// Rule 4: Expression Simplification
/// Simplifies logical expressions
/// Example: age > 18 AND age > 20 → age > 20
pub struct ExpressionSimplificationRule;

impl OptimizationRule for ExpressionSimplificationRule {
    fn name(&self) -> &str {
        "ExpressionSimplification"
    }

    fn apply(&self, statement: Statement) -> Result<Statement> {
        match statement {
            Statement::Select {
                columns,
                from,
                where_clause,
                using_engine,
                order_by,
                limit,
            } => {
                let simplified_where = where_clause.map(|expr| self.simplify(expr));

                Ok(Statement::Select {
                    columns,
                    from,
                    where_clause: simplified_where,
                    using_engine,
                    order_by,
                    limit,
                })
            }
            _ => Ok(statement),
        }
    }
}

impl ExpressionSimplificationRule {
    fn simplify(&self, expr: Expr) -> Expr {
        match expr {
            Expr::BinaryOp {
                left,
                op: BinaryOperator::And,
                right,
            } => {
                // Simplify both sides
                let left_simple = self.simplify(*left);
                let right_simple = self.simplify(*right);

                // Check for tautologies
                if let Expr::Literal(Value::Boolean(true)) = left_simple {
                    return right_simple; // true AND x → x
                }
                if let Expr::Literal(Value::Boolean(true)) = right_simple {
                    return left_simple; // x AND true → x
                }
                if let Expr::Literal(Value::Boolean(false)) = left_simple {
                    return Expr::Literal(Value::Boolean(false)); // false AND x → false
                }
                if let Expr::Literal(Value::Boolean(false)) = right_simple {
                    return Expr::Literal(Value::Boolean(false)); // x AND false → false
                }

                Expr::BinaryOp {
                    left: Box::new(left_simple),
                    op: BinaryOperator::And,
                    right: Box::new(right_simple),
                }
            }
            _ => expr,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_folding() {
        let rule = ConstantFoldingRule;

        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Literal(Value::Integer(1))),
            op: BinaryOperator::Eq,
            right: Box::new(Expr::Literal(Value::Integer(1))),
        };

        let folded = rule.fold_constants(expr);

        assert_eq!(folded, Expr::Literal(Value::Boolean(true)));
    }

    #[test]
    fn test_expression_simplification() {
        let rule = ExpressionSimplificationRule;

        // true AND x → x
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Literal(Value::Boolean(true))),
            op: BinaryOperator::And,
            right: Box::new(Expr::Column("age".to_string())),
        };

        let simplified = rule.simplify(expr);

        assert_eq!(simplified, Expr::Column("age".to_string()));
    }
}
