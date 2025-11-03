//! Intelligent Query Router
//! PROPRIETARY & PATENTABLE INNOVATION #3

use crate::engine::{Query, StorageEngine, QueryType};
use std::collections::HashMap;

/// Smart query router with cost-based optimization
pub struct QueryRouter {
    /// Performance history for learning
    history: HashMap<String, Vec<f64>>,
}

impl QueryRouter {
    pub fn new() -> Self {
        Self {
            history: HashMap::new(),
        }
    }
    
    /// Analyze query to determine type
    pub fn analyze_query(&self, query: &Query) -> QueryType {
        let sql_upper = query.sql.to_uppercase();
        
        if sql_upper.contains("SEMANTIC_SEARCH") || sql_upper.contains("MATCHES") {
            QueryType::SemanticSearch
        } else if sql_upper.contains("GRAPH_WALK") || sql_upper.contains("TRAVERSE") {
            QueryType::GraphQuery
        } else if sql_upper.contains("REASON") || sql_upper.contains("NEURAL_REASON") {
            QueryType::Reasoning
        } else if sql_upper.contains("CLASSIFY") || sql_upper.contains("HDC_") {
            QueryType::SemanticSearch  // HDC also does semantic
        } else {
            QueryType::BasicSQL
        }
    }
    
    /// Find best engine for query
    pub fn route(&self, query: &Query, engines: &HashMap<String, Box<dyn StorageEngine>>) -> String {
        let query_type = self.analyze_query(query);
        
        // Check for explicit engine hint
        if query.sql.to_uppercase().contains("USING colbert") {
            return "colbert".to_string();
        }
        if query.sql.to_uppercase().contains("USING hdc") {
            return "hdc".to_string();
        }
        
        // Find best engine by capabilities and cost
        let mut best_engine = "default".to_string();
        let mut best_cost = f64::MAX;
        
        for (name, engine) in engines {
            let caps = engine.capabilities();
            
            // Check if engine can handle this query type
            if caps.can_handle(&query_type) {
                let cost = engine.estimate_cost(query);
                let total_cost = cost.total_score();
                
                if total_cost < best_cost {
                    best_cost = total_cost;
                    best_engine = name.clone();
                }
            }
        }
        
        tracing::debug!("Routed query type {:?} to engine: {}", query_type, best_engine);
        best_engine
    }
    
    /// Record query performance for learning
    pub fn record_performance(&mut self, engine: &str, latency_ms: f64) {
        self.history
            .entry(engine.to_string())
            .or_insert_with(Vec::new)
            .push(latency_ms);
    }
    
    /// Get average performance for engine
    pub fn get_avg_performance(&self, engine: &str) -> Option<f64> {
        self.history.get(engine).map(|perf| {
            perf.iter().sum::<f64>() / perf.len() as f64
        })
    }
}
