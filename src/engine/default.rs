//! Default SQL Engine
//! PROPRIETARY

use crate::engine::{EngineCapabilities, Executor, Query, QueryCost, StorageEngine};
use crate::query::evaluator::QueryParams;
use crate::query::QueryParser;
use crate::storage::disk::DiskStorage;
use crate::types::ResultSet;
use crate::Result;
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

pub struct DefaultEngine {
    name: String,
    executor: Arc<RwLock<Executor>>,
    disk: Arc<DiskStorage>,
    is_memory: bool,

    /// When true, suppress per-statement `disk.save_table()` calls so bulk
    /// loads don't pay O(table-size) serialization on every INSERT. Tables
    /// touched while in bulk mode are tracked in `dirty_tables` and flushed
    /// together when bulk mode turns off or `flush()` is called.
    bulk_mode: Arc<AtomicBool>,
    dirty_tables: Arc<Mutex<HashSet<String>>>,
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
            bulk_mode: Arc::new(AtomicBool::new(false)),
            dirty_tables: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    /// Toggle bulk mode. When ON, DDL/DML statements skip the per-statement
    /// `save_table` and just accumulate dirty-table names. When OFF (or on
    /// `flush()`), all dirty tables are serialized once.
    pub fn set_bulk_mode(&self, enabled: bool) {
        self.bulk_mode.store(enabled, Ordering::SeqCst);
    }

    /// Flush any dirty tables to disk. Idempotent — safe to call at any time.
    pub async fn flush(&self) -> Result<()> {
        if self.is_memory {
            return Ok(());
        }
        let mut dirty = self.dirty_tables.lock().await;
        let executor = self.executor.read().await;
        for name in dirty.iter() {
            if let Some(table) = executor.get_tables().get(name) {
                self.disk.save_table(table)?;
            }
        }
        dirty.clear();
        Ok(())
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

        // Convert Vec<Value> params from query to QueryParams
        let params = QueryParams::with_positional(query.params.clone());

        let mut executor = self.executor.write().await;
        let result = executor.execute(&statement, &params)?;

        // Skip disk writes for in-memory mode
        if !self.is_memory {
            let touched_table = match &statement {
                crate::query::Statement::CreateTable { name, .. }
                | crate::query::Statement::Insert { table: name, .. }
                | crate::query::Statement::Update { table: name, .. }
                | crate::query::Statement::Delete { from: name, .. } => Some(name.clone()),
                _ => None,
            };

            if let Some(name) = touched_table {
                if self.bulk_mode.load(Ordering::SeqCst) {
                    // Bulk mode: just remember this table is dirty, don't serialize now
                    let mut dirty = self.dirty_tables.lock().await;
                    dirty.insert(name);
                } else {
                    // Normal mode: persist immediately (pre-existing semantics)
                    if let Some(table) = executor.get_tables().get(&name) {
                        self.disk.save_table(table)?;
                    }
                }
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

    fn set_bulk_mode(&self, enabled: bool) {
        // Inherent method handles the state; trait dispatch just forwards.
        DefaultEngine::set_bulk_mode(self, enabled)
    }

    async fn flush(&self) -> Result<()> {
        DefaultEngine::flush(self).await
    }
}
