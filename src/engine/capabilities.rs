//! Engine Capabilities
//! PROPRIETARY

use serde::{Deserialize, Serialize};

/// What an engine can do
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineCapabilities {
    /// Supports semantic similarity search
    pub semantic_search: bool,
    
    /// Supports graph traversal queries
    pub graph_traversal: bool,
    
    /// Supports logical reasoning
    pub reasoning: bool,
    
    /// Optimized for time-series data
    pub time_series: bool,
    
    /// Basic SQL support
    pub sql: bool,
    
    /// Energy efficiency mode
    pub low_power: bool,
    
    /// Requires GPU
    pub gpu_required: bool,
}

impl Default for EngineCapabilities {
    fn default() -> Self {
        Self {
            semantic_search: false,
            graph_traversal: false,
            reasoning: false,
            time_series: false,
            sql: true,  // Default to basic SQL
            low_power: false,
            gpu_required: false,
        }
    }
}

impl EngineCapabilities {
    pub fn can_handle(&self, query_type: &QueryType) -> bool {
        match query_type {
            QueryType::SemanticSearch => self.semantic_search,
            QueryType::GraphQuery => self.graph_traversal,
            QueryType::Reasoning => self.reasoning,
            QueryType::TimeSeries => self.time_series,
            QueryType::BasicSQL => self.sql,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum QueryType {
    SemanticSearch,
    GraphQuery,
    Reasoning,
    TimeSeries,
    BasicSQL,
}
