//! SQL transaction boundaries for the default engine's actual disk path.
//! The engine holds its executor write lock for every call, including checkpoint.

use crate::engine::Executor;
use crate::query::ast::{Statement, TransactionOp};
use crate::query::evaluator::QueryParams;
use crate::storage::disk::DiskStorage;
use crate::storage::wal::SyncPolicy;
use crate::storage::Table;
use crate::types::{ResultSet, Row, Value};
use crate::{NexusError, Result};
use std::collections::{HashMap, HashSet};

pub(crate) struct DurableExecutor {
    executor: Executor,
    disk: DiskStorage,
    active: bool,
    dirty: HashSet<String>,
}

impl DurableExecutor {
    pub fn new(path: &str, is_memory: bool) -> Result<Self> {
        let disk = DiskStorage::new(if is_memory { ":memory:" } else { path })?;
        let mut executor = Executor::new();
        for name in disk.list_tables()? {
            if let Some(table) = disk.load_table(&name)? {
                executor.get_tables_mut().insert(name, table);
            }
        }
        Ok(Self {
            executor,
            disk,
            active: false,
            dirty: HashSet::new(),
        })
    }

    pub fn get_tables(&self) -> &HashMap<String, Table> {
        self.executor.get_tables()
    }

    pub fn execute(
        &mut self,
        statement: &Statement,
        params: &QueryParams,
        bulk: bool,
    ) -> Result<ResultSet> {
        if self.disk.is_memory() {
            return self.executor.execute(statement, params);
        }
        if let Statement::Pragma { name, value } = statement {
            if let Some(result) = self.pragma(name, value.as_deref())? {
                return Ok(result);
            }
        }
        if let Statement::Transaction { operation } = statement {
            match operation {
                TransactionOp::Begin if !self.active => {
                    // Deferred autocommits predating BEGIN belong to the old
                    // state and must not be accidentally rolled back with it.
                    self.commit_dirty()?;
                    let result = self.executor.execute(statement, params)?;
                    self.active = true;
                    return Ok(result);
                }
                TransactionOp::Commit if self.active => {
                    // Keep the rollback snapshot until the WAL commit succeeds.
                    // A failed sync leaves the transaction active and reports IO.
                    self.commit_dirty()?;
                    let result = self.executor.execute(statement, params)?;
                    self.active = false;
                    return Ok(result);
                }
                TransactionOp::Rollback if self.active => {
                    let result = self.executor.execute(statement, params)?;
                    self.dirty.clear();
                    self.active = false;
                    return Ok(result);
                }
                _ => return self.executor.execute(statement, params),
            }
        }

        let touched = match statement {
            Statement::CreateTable { name, .. }
            | Statement::DropTable { name, .. }
            | Statement::CreateIndex { table: name, .. }
            | Statement::Insert { table: name, .. }
            | Statement::Update { table: name, .. }
            | Statement::Delete { from: name, .. } => Some(name.as_str()),
            _ => None,
        };
        let before = touched.and_then(|name| self.get_tables().get(name).cloned());
        match self.executor.execute(statement, params) {
            Ok(result) => {
                if let Some(name) = touched {
                    self.finish_write(name, before, bulk)?;
                }
                Ok(result)
            }
            Err(error) => {
                if let Some(name) = touched {
                    self.restore(name, before);
                }
                Err(error)
            }
        }
    }

    /// The fast literal INSERT and Python bulk API use this same transaction
    /// boundary, even though they bypass SQL parsing and Executor::execute.
    pub fn bulk_insert(
        &mut self,
        name: &str,
        columns: &[String],
        rows: Vec<Vec<Value>>,
        bulk: bool,
    ) -> Result<usize> {
        let table = self
            .executor
            .get_tables_mut()
            .get_mut(name)
            .ok_or_else(|| NexusError::QueryError(format!("Table {} not found", name)))?;
        let col_map: Vec<_> = columns
            .iter()
            .map(|name| table.columns.iter().position(|c| c == name))
            .collect();
        // Validate/coerce the entire statement before changing any row/index.
        let mut prepared = Vec::with_capacity(rows.len());
        for raw in rows {
            let mut values = vec![Value::Null; table.columns.len()];
            for (i, value) in raw.into_iter().enumerate() {
                if let Some(Some(col)) = col_map.get(i) {
                    values[*col] = value;
                }
            }
            table.coerce_values(&mut values)?;
            prepared.push(Row::new(table.columns.clone(), values));
        }
        let before = (!self.disk.is_memory() && !self.active && !bulk).then(|| table.clone());
        let count = prepared.len();
        table.rows.reserve(count);
        // Batch insert so geometry GiST indexes bulk-load instead of inserting one by one.
        table.insert_batch(prepared);
        if !self.disk.is_memory() {
            self.finish_write(name, before, bulk)?;
        }
        Ok(count)
    }

    fn finish_write(&mut self, name: &str, before: Option<Table>, bulk: bool) -> Result<()> {
        let already_dirty = !self.dirty.insert(name.into());
        if !self.active && !bulk {
            if let Err(error) = self.commit_dirty() {
                self.restore(name, before);
                if !already_dirty {
                    self.dirty.remove(name);
                }
                return Err(error);
            }
        }
        Ok(())
    }

    fn restore(&mut self, name: &str, before: Option<Table>) {
        if let Some(table) = before {
            self.executor.get_tables_mut().insert(name.into(), table);
        } else {
            self.executor.get_tables_mut().remove(name);
        }
    }

    fn commit_dirty(&mut self) -> Result<()> {
        let mut names: Vec<_> = self.dirty.iter().collect();
        names.sort();
        let mut tables = Vec::new();
        let mut removed = Vec::new();
        for name in names {
            if let Some(table) = self.get_tables().get(name) {
                tables.push(table);
            } else {
                removed.push(name.clone());
            }
        }
        self.disk.commit_tables(&tables, &removed)?;
        self.dirty.clear();
        Ok(())
    }

    /// Flush is never an implicit COMMIT of an active SQL transaction.
    pub fn flush(&mut self) -> Result<()> {
        if !self.active {
            self.commit_dirty()?;
        }
        self.disk.sync()
    }

    pub fn close(&mut self) -> Result<()> {
        if self.active {
            self.executor.execute(
                &Statement::Transaction {
                    operation: TransactionOp::Rollback,
                },
                &QueryParams::default(),
            )?;
            self.active = false;
            self.dirty.clear();
        }
        self.flush()
    }

    fn pragma(&mut self, name: &str, value: Option<&str>) -> Result<Option<ResultSet>> {
        let name = name.to_ascii_lowercase();
        let result = match name.as_str() {
            "synchronous" => {
                if let Some(value) = value {
                    if self.active {
                        return Err(NexusError::ConfigError(
                            "Cannot change synchronous inside a transaction".into(),
                        ));
                    }
                    self.disk.set_sync_policy(SyncPolicy::parse(value)?)?;
                }
                Value::Integer(self.disk.sync_policy().as_integer())
            }
            "journal_mode" => {
                if value.is_some_and(|v| !v.trim_matches(['\'', '"']).eq_ignore_ascii_case("wal")) {
                    return Err(NexusError::ConfigError(
                        "Disk storage requires WAL journaling".into(),
                    ));
                }
                Value::Text("WAL".into())
            }
            "nexusql_wal_checkpoint_bytes" => {
                if let Some(value) = value {
                    let bytes = value
                        .parse::<i64>()
                        .ok()
                        .filter(|v| *v >= 0)
                        .ok_or_else(|| {
                            NexusError::ConfigError(
                                "Checkpoint threshold must be nonnegative bytes".into(),
                            )
                        })?;
                    self.disk.set_checkpoint_bytes(bytes as u64);
                }
                Value::Integer(self.disk.checkpoint_bytes() as i64)
            }
            "wal_checkpoint" => {
                // Checkpoint only what is already committed, including during BEGIN.
                self.disk.checkpoint()?;
                Value::Integer(0)
            }
            _ => return Ok(None),
        };
        Ok(Some(ResultSet::new(vec![Row::new(
            vec![name],
            vec![result],
        )])))
    }
}
