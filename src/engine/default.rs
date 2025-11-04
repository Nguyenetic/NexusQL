//! Default SQL Engine
//! PROPRIETARY

use crate::engine::{EngineCapabilities, Executor, Query, QueryCost, StorageEngine};
use crate::query::QueryParser;
use crate::storage::disk::DiskStorage;
use crate::types::ResultSet;
use crate::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct DefaultEngine {
    name: String,
    executor: Arc<RwLock<Executor>>,
    disk: Arc<DiskStorage>,
    is_memory: bool,
}

impl DefaultEngine {
    pub fn new(data_path: &str, is_memory: bool) -> Result<Self> {
        let disk = DiskStorage::new(data_path)?;
        let mut executor = Executor::new();

        // Skip loading tables from disk for in-memory mode
        if !is_memory {
            let table_names = disk.list_tables()?;
            for table_name in table_names {
                if let Some(table) = disk.load_table(&table_name)? {
                    executor.get_tables_mut().insert(table_name, table);
                }
            }
        }

        Ok(Self {
            name: "default".to_string(),
            executor: Arc::new(RwLock::new(executor)),
            disk: Arc::new(disk),
            is_memory,
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

        // Skip disk writes for in-memory mode
        if !self.is_memory {
            match statement {
                crate::query::Statement::CreateTable { ref name, .. }
                | crate::query::Statement::Insert {
                    table: ref name, ..
                }
                | crate::query::Statement::Update {
                    table: ref name, ..
                }
                | crate::query::Statement::Delete { from: ref name, .. } => {
                    if let Some(table) = executor.get_tables().get(name) {
                        self.disk.save_table(table)?;
                    }
                }
                _ => {}
            }
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
