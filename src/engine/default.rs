//! Default SQL Engine
//! PROPRIETARY

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::engine::{StorageEngine, Query, QueryCost, EngineCapabilities, Executor};
use crate::query::{QueryParser};
use crate::storage::{disk::DiskStorage};
use crate::types::{ResultSet};
use crate::Result;

pub struct DefaultEngine {
    name: String,
    executor: Arc<RwLock<Executor>>,
    disk: Arc<DiskStorage>,
}

impl DefaultEngine {
    pub fn new(data_path: &str) -> Result<Self> {
        let disk = DiskStorage::new(data_path)?;
        let mut executor = Executor::new();
        
        let table_names = disk.list_tables()?;
        for table_name in table_names {
            if let Some(table) = disk.load_table(&table_name)? {
                executor.get_tables_mut().insert(table_name, table);
            }
        }
        
        Ok(Self {
            name: "default".to_string(),
            executor: Arc::new(RwLock::new(executor)),
            disk: Arc::new(disk),
        })
    }
}

#[async_trait]
impl StorageEngine for DefaultEngine {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities {
            sql: true,
            ..Default::default()
        }
    }
    
    async fn execute(&self, query: &Query) -> Result<ResultSet> {
        let statement = QueryParser::parse(&query.sql)?;
        
        let mut executor = self.executor.write().await;
        let result = executor.execute(&statement)?;
        
        match statement {
            crate::query::Statement::CreateTable { ref name, .. } |
            crate::query::Statement::Insert { table: ref name, .. } |
            crate::query::Statement::Update { table: ref name, .. } |
            crate::query::Statement::Delete { from: ref name, .. } => {
                if let Some(table) = executor.get_tables().get(name) {
                    self.disk.save_table(table)?;
                }
            },
            _ => {},
        }
        
        Ok(result)
    }
    
    fn estimate_cost(&self, _query: &Query) -> QueryCost {
        QueryCost {
            latency_ms: 2.0,
            memory_mb: 10.0,
            energy_mw: 100.0,
            accuracy: 1.0,
        }
    }
    
    async fn initialize(&mut self) -> Result<()> {
        let executor = self.executor.read().await;
        let table_count = executor.get_tables().len();
        tracing::info!("DefaultEngine initialized ({} tables)", table_count);
        Ok(())
    }
}
