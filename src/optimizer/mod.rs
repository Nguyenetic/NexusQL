//! Query Optimizer - DuckDB-inspired
//! PROPRIETARY

pub mod rules;
pub mod pushdown;

use crate::query::ast::Statement;
use crate::Result;

/// Query optimizer
pub struct Optimizer {
    rules: Vec<Box<dyn OptimizationRule>>,
}

impl Optimizer {
    pub fn new() -> Self {
        let mut rules: Vec<Box<dyn OptimizationRule>> = vec![];
        
        // Add optimization rules (order matters!)
        rules.push(Box::new(rules::ConstantFoldingRule));
        rules.push(Box::new(rules::FilterPushdownRule));
        rules.push(Box::new(rules::ProjectionPushdownRule));
        rules.push(Box::new(rules::ExpressionSimplificationRule));
        
        Self { rules }
    }
    
    /// Optimize a statement
    pub fn optimize(&self, statement: Statement) -> Result<Statement> {
        let mut optimized = statement;
        
        // Apply each optimization rule
        for rule in &self.rules {
            optimized = rule.apply(optimized)?;
        }
        
        Ok(optimized)
    }
}

/// Optimization rule trait
pub trait OptimizationRule: Send + Sync {
    fn name(&self) -> &str;
    fn apply(&self, statement: Statement) -> Result<Statement>;
}
