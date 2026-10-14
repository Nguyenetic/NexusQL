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
        // One fsync for the whole batch — sled auto-flushes on drop so this
        // is defensive (durability on explicit flush vs process exit).
        self.disk.sync()?;
        Ok(())
    }

    /// Direct bulk insert — completely bypasses the SQL parser + executor.
    /// Appends `rows` to the named table, mapping positional values to the
    /// supplied `columns`. Respects bulk_mode for write batching.
    ///
    /// This is the hot path for data-loading workloads: saves ~4 ms per
    /// statement (sqlparser overhead) and avoids round-tripping through
    /// the query router + optimizer.
    pub async fn bulk_insert(
        &self,
        table: &str,
        columns: &[String],
        rows: Vec<Vec<crate::types::Value>>,
    ) -> Result<usize> {
        use crate::types::{Row, Value};

        let mut executor = self.executor.write().await;
        let tables = executor.get_tables_mut();
        let t = tables
            .get_mut(table)
            .ok_or_else(|| crate::NexusError::QueryError(format!("Table {} not found", table)))?;

        // Build a positional mapping once: column-index-in-input -> column-index-in-table.
        let mut col_map: Vec<Option<usize>> = Vec::with_capacity(columns.len());
        for name in columns {
            let idx = t.columns.iter().position(|c| c == name);
            col_map.push(idx);
        }

        let count = rows.len();
        // Reserve capacity to avoid rebuilds
        t.rows.reserve(count);

        for raw in rows {
            let mut row_values = vec![Value::Null; t.columns.len()];
            for (i, v) in raw.into_iter().enumerate() {
                if let Some(Some(table_col)) = col_map.get(i).copied() {
                    row_values[table_col] = v;
                }
            }
            // Row keeps column names alongside values in the current schema;
            // that's wasteful but a separate refactor. Reuse the slice once
            // to keep the clone cheap.
            t.coerce_values(&mut row_values)?;
            t.insert(Row::new(t.columns.clone(), row_values));
        }

        // Mark dirty so flush() writes this table
        if !self.is_memory {
            let mut dirty = self.dirty_tables.lock().await;
            dirty.insert(table.to_string());

            // If bulk mode is OFF, commit now
            if !self.bulk_mode.load(Ordering::SeqCst) {
                if let Some(tbl) = tables.get(table) {
                    self.disk.save_table(tbl)?;
                }
                dirty.remove(table);
            }
        }

        Ok(count)
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
                | crate::query::Statement::CreateIndex { table: name, .. }
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

    async fn bulk_insert(
        &self,
        table: &str,
        columns: &[String],
        rows: Vec<Vec<crate::types::Value>>,
    ) -> Result<Option<usize>> {
        Ok(Some(
            DefaultEngine::bulk_insert(self, table, columns, rows).await?,
        ))
    }

    async fn table_columns(&self, table: &str) -> Result<Option<Vec<String>>> {
        let executor = self.executor.read().await;
        Ok(executor.get_tables().get(table).map(|t| t.columns.clone()))
    }
}
