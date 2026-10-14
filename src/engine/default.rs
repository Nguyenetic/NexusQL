//! Default SQL Engine
//! PROPRIETARY

use crate::engine::{EngineCapabilities, Query, QueryCost, StorageEngine};
use crate::query::evaluator::QueryParams;
use crate::query::QueryParser;
use crate::transaction::durable::DurableExecutor;
use crate::types::ResultSet;
use crate::Result;
use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct DefaultEngine {
    name: String,
    executor: Arc<RwLock<DurableExecutor>>,

    /// Defer autocommit persistence until flush. Explicit COMMIT is always
    /// durable according to the configured sync policy, including in bulk mode.
    bulk_mode: Arc<AtomicBool>,
}

impl DefaultEngine {
    pub fn new(data_path: &str, is_memory: bool) -> Result<Self> {
        Ok(Self {
            name: "default".to_string(),
            executor: Arc::new(RwLock::new(DurableExecutor::new(data_path, is_memory)?)),
            bulk_mode: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Toggle bulk mode. Call flush() to persist deferred autocommits.
    pub fn set_bulk_mode(&self, enabled: bool) {
        self.bulk_mode.store(enabled, Ordering::SeqCst);
    }

    /// Flush any dirty tables to disk. Idempotent — safe to call at any time.
    pub async fn flush(&self) -> Result<()> {
        self.executor.write().await.flush()
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
        self.executor.write().await.bulk_insert(
            table,
            columns,
            rows,
            self.bulk_mode.load(Ordering::SeqCst),
        )
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

        self.executor.write().await.execute(
            &statement,
            &params,
            self.bulk_mode.load(Ordering::SeqCst),
        )
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

    async fn shutdown(&mut self) -> Result<()> {
        self.executor.write().await.close()
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
