//! NexusSQL Micro-Kernel - With SQL Hints Support
//! PROPRIETARY

use crate::engine::{Query, StorageEngine};
use crate::query::extract_engine_hint;
use crate::router::QueryRouter;
use crate::types::ResultSet;
use crate::{NexusError, Result};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[allow(dead_code)]
pub struct NexusKernel {
    engines: Arc<DashMap<String, Box<dyn StorageEngine>>>,
    router: Arc<RwLock<QueryRouter>>,
    default_engine: String,
    data_path: String,
    is_memory: bool,
}

impl NexusKernel {
    pub async fn new(data_path: &str, is_memory: bool) -> Result<Self> {
        tracing::info!(
            "Initializing NexusSQL micro-kernel (mode: {})",
            if is_memory { "memory" } else { "disk" }
        );

        let kernel = Self {
            engines: Arc::new(DashMap::new()),
            router: Arc::new(RwLock::new(QueryRouter::new())),
            default_engine: "default".to_string(),
            data_path: data_path.to_string(),
            is_memory,
        };

        tracing::info!("✓ Micro-kernel initialized at {}", data_path);
        Ok(kernel)
    }

    pub fn is_memory(&self) -> bool {
        self.is_memory
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

    pub async fn execute_with_params(
        &self,
        sql: &str,
        params: Vec<crate::types::Value>,
    ) -> Result<ResultSet> {
        let query = Query {
            sql: sql.to_string(),
            params,
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
        let engine = self
            .engines
            .get(&engine_name)
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

    pub async fn execute(&self, sql: &str) -> Result<ResultSet> {
        self.execute_with_params(sql, vec![]).await
    }

    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down NexusSQL kernel");
        // Ensure any dirty buffers land on disk before the engines go away
        self.flush().await.ok();
        for mut engine in self.engines.iter_mut() {
            engine.value_mut().shutdown().await?;
        }
        Ok(())
    }

    /// Propagate bulk-mode toggle to every engine. No-op for engines that
    /// don't override the trait default.
    pub async fn set_bulk_mode(&self, enabled: bool) {
        for engine in self.engines.iter() {
            engine.value().set_bulk_mode(enabled);
        }
    }

    /// Flush any deferred writes across every engine.
    pub async fn flush(&self) -> Result<()> {
        for engine in self.engines.iter() {
            engine.value().flush().await?;
        }
        Ok(())
    }

    /// Look up the declared columns for a table on the default engine.
    /// Used by fast-path INSERT parsing to resolve implicit column lists.
    pub async fn table_columns(&self, table: &str) -> Result<Option<Vec<String>>> {
        let engine = self
            .engines
            .get("default")
            .ok_or_else(|| NexusError::EngineNotFound("default".into()))?;
        engine.value().table_columns(table).await
    }

    /// Direct bulk insert bypassing SQL parse. Routes to the default engine
    /// (the only one that currently implements the fast path).
    pub async fn bulk_insert(
        &self,
        table: &str,
        columns: &[String],
        rows: Vec<crate::types::Value>,
        row_count: usize,
    ) -> Result<usize> {
        let engine = self
            .engines
            .get("default")
            .ok_or_else(|| NexusError::EngineNotFound("default".into()))?;
        let ncols = columns.len();
        let mut row_vecs: Vec<Vec<crate::types::Value>> = Vec::with_capacity(row_count);
        let mut it = rows.into_iter();
        for _ in 0..row_count {
            let mut row = Vec::with_capacity(ncols);
            for _ in 0..ncols {
                row.push(it.next().unwrap_or(crate::types::Value::Null));
            }
            row_vecs.push(row);
        }
        match engine.bulk_insert(table, columns, row_vecs).await? {
            Some(n) => Ok(n),
            None => Err(NexusError::QueryError(
                "default engine does not support bulk_insert".into(),
            )),
        }
    }
}
