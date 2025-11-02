//! NexusSQL Micro-Kernel - With SQL Hints Support
//! PROPRIETARY

use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::RwLock;
use crate::engine::{StorageEngine, Query};
use crate::types::ResultSet;
use crate::router::QueryRouter;
use crate::query::extract_engine_hint;
use crate::{Result, NexusError};

pub struct NexusKernel {
    engines: Arc<DashMap<String, Box<dyn StorageEngine>>>,
    router: Arc<RwLock<QueryRouter>>,
    default_engine: String,
    data_path: String,
}

impl NexusKernel {
    pub async fn new(data_path: &str) -> Result<Self> {
        tracing::info!("Initializing NexusSQL micro-kernel");
        
        let kernel = Self {
            engines: Arc::new(DashMap::new()),
            router: Arc::new(RwLock::new(QueryRouter::new())),
            default_engine: "default".to_string(),
            data_path: data_path.to_string(),
        };
        
        tracing::info!("✓ Micro-kernel initialized at {}", data_path);
        Ok(kernel)
    }
    
    pub async fn register_engine(&self, mut engine: Box<dyn StorageEngine>) -> Result<()> {
        let name = engine.name().to_string();
        engine.initialize().await?;
        self.engines.insert(name.clone(), engine);
        tracing::info!("✓ Registered engine: {}", name);
        Ok(())
    }
    
    pub fn list_engines(&self) -> Vec<String> {
        self.engines.iter().map(|e| e.key().clone()).collect()
    }
    
    pub async fn execute(&self, sql: &str) -> Result<ResultSet> {
        let query = Query {
            sql: sql.to_string(),
            params: vec![],
        };
        
        // Check for engine hint in SQL comment
        let engine_name = if let Some(hint) = extract_engine_hint(sql) {
            hint
        } else {
            // Use router to select engine
            let router = self.router.read().await;
            router.route(&query, &std::collections::HashMap::new())
        };
        
        // Get engine and execute
        let engine = self.engines.get(&engine_name)
            .ok_or_else(|| NexusError::EngineNotFound(engine_name.clone()))?;
        
        tracing::debug!("→ Routing to: {}", engine_name);
        
        let start = std::time::Instant::now();
        let result = engine.execute(&query).await?;
        let latency = start.elapsed().as_secs_f64() * 1000.0;
        
        // Record performance
        let mut router = self.router.write().await;
        router.record_performance(&engine_name, latency);
        
        Ok(result)
    }
    
    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down NexusSQL kernel");
        for mut engine in self.engines.iter_mut() {
            engine.value_mut().shutdown().await?;
        }
        Ok(())
    }
}
