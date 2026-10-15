//! SQL transaction boundaries for the default engine's actual disk path.
//! The engine holds its executor write lock for every call, including checkpoint.

use crate::engine::Executor;
use crate::query::ast::{OnConflict, Statement, TransactionOp};
use crate::query::evaluator::QueryParams;
use crate::storage::disk::DiskStorage;
use crate::storage::wal::SyncPolicy;
use crate::storage::Table;
use crate::types::{ResultSet, Row, Value};
use crate::{NexusError, Result};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// How to undo one statement if it, or its autocommit, fails.
enum Undo {
    /// The table as it was (None: it did not exist).
    Snapshot(Option<Box<Table>>),
    /// The statement only appended rows: truncate back instead of cloning
    /// the whole table (and its indexes) before every INSERT.
    Append {
        rows: usize,
        next_auto_increment_id: i64,
    },
}

impl Undo {
    fn append(table: &Table) -> Self {
        Undo::Append {
            rows: table.rows.len(),
            next_auto_increment_id: table.next_auto_increment_id,
        }
    }
}

pub(crate) struct DurableExecutor {
    executor: Executor,
    disk: Arc<DiskStorage>,
    changefeeds: crate::changefeed::Changefeeds,
    active: bool,
    dirty: HashSet<String>,
}

impl DurableExecutor {
    pub fn new(path: &str, is_memory: bool) -> Result<Self> {
        let disk = Arc::new(DiskStorage::new(if is_memory { ":memory:" } else { path })?);
        let mut executor = Executor::new();
        for name in disk.list_tables()? {
            if name == crate::changefeed::LOG_TABLE {
                continue;
            }
            if let Some(table) = disk.load_table(&name)? {
                executor.get_tables_mut().insert(name, table);
            }
        }
        let changefeeds =
            crate::changefeed::Changefeeds::open(disk.clone(), executor.get_tables())?;
        Ok(Self {
            executor,
            disk,
            changefeeds,
            active: false,
            dirty: HashSet::new(),
        })
    }

    pub fn get_tables(&self) -> &HashMap<String, Table> {
        self.executor.get_tables()
    }

    pub fn changefeeds(&self) -> crate::changefeed::Changefeeds {
        self.changefeeds.clone()
    }

    pub fn execute(
        &mut self,
        statement: &Statement,
        params: &QueryParams,
        bulk: bool,
    ) -> Result<ResultSet> {
        self.changefeeds.validate_statement(statement)?;
        if let Statement::Changefeed(command) = statement {
            if !self.active {
                self.commit_dirty()?;
            }
            self.changefeeds
                .manage(command, self.get_tables(), self.active)?;
            return Ok(ResultSet::empty());
        }
        if !self.disk.is_memory() {
            if let Statement::Pragma { name, value } = statement {
                if let Some(result) = self.pragma(name, value.as_deref())? {
                    return Ok(result);
                }
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
                    self.changefeeds.begin_transaction();
                    return Ok(result);
                }
                TransactionOp::Commit if self.active => {
                    // Keep the rollback snapshot until the WAL commit succeeds.
                    // A failed sync leaves the transaction active and reports IO.
                    self.commit_dirty()?;
                    let result = self.executor.execute(statement, params)?;
                    self.active = false;
                    self.changefeeds.end_transaction();
                    return Ok(result);
                }
                TransactionOp::Rollback if self.active => {
                    let result = self.executor.execute(statement, params)?;
                    self.dirty.clear();
                    self.active = false;
                    self.changefeeds.rollback();
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
        let before = match (
            statement,
            touched.and_then(|name| self.get_tables().get(name)),
        ) {
            (
                Statement::Insert {
                    on_conflict:
                        None
                        | Some(
                            OnConflict::Ignore
                            | OnConflict::Abort
                            | OnConflict::Fail
                            | OnConflict::Rollback,
                        ),
                    ..
                },
                Some(table),
            ) => Undo::append(table),
            (_, table) => Undo::Snapshot(table.cloned().map(Box::new)),
        };
        match self.executor.execute(statement, params) {
            Ok(result) => {
                if let Some(name) = touched {
                    self.finish_write(
                        name,
                        before,
                        bulk,
                        matches!(statement, Statement::Update { .. }),
                    )?;
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
            prepared.push(values);
        }
        // Validation passed; ids are only consumed by rows that are inserted.
        let before = Undo::append(table);
        let prepared: Vec<Row> = prepared
            .into_iter()
            .map(|mut values| {
                table.assign_auto_increment(&mut values);
                Row::new(table.columns.clone(), values)
            })
            .collect();
        let count = prepared.len();
        table.rows.reserve(count);
        // Batch insert so geometry GiST indexes bulk-load instead of inserting one by one.
        table.insert_batch(prepared);
        self.finish_write(name, before, bulk, false)?;
        Ok(count)
    }

    fn finish_write(
        &mut self,
        name: &str,
        before: Undo,
        bulk: bool,
        preserve_positions: bool,
    ) -> Result<()> {
        let capture =
            match self
                .changefeeds
                .capture_table(name, preserve_positions, self.get_tables())
            {
                Ok(capture) => capture,
                Err(error) => {
                    self.restore(name, before);
                    return Err(error);
                }
            };
        let already_dirty = !self.dirty.insert(name.into());
        if !self.active && !bulk {
            if let Err(error) = self.commit_dirty() {
                self.restore(name, before);
                self.changefeeds.undo_capture(capture);
                if !already_dirty {
                    self.dirty.remove(name);
                }
                return Err(error);
            }
        }
        Ok(())
    }

    fn restore(&mut self, name: &str, before: Undo) {
        match before {
            Undo::Snapshot(Some(table)) => {
                self.executor.get_tables_mut().insert(name.into(), *table);
            }
            Undo::Snapshot(None) => {
                self.executor.get_tables_mut().remove(name);
            }
            Undo::Append {
                rows,
                next_auto_increment_id,
            } => {
                if let Some(table) = self.executor.get_tables_mut().get_mut(name) {
                    table.truncate_rows(rows);
                    table.next_auto_increment_id = next_auto_increment_id;
                }
            }
        }
    }

    fn commit_dirty(&mut self) -> Result<()> {
        let prepared = self.changefeeds.prepare_commit()?;
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
        if let Some(log) = prepared.table() {
            tables.push(log);
        }
        self.disk.commit_tables(&tables, &removed)?;
        prepared.post_commit();
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
            self.changefeeds.rollback();
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

#[cfg(test)]
mod changefeed_tests {
    use super::*;
    use crate::changefeed::{ChangeEvent, Operation};
    use crate::query::QueryParser;
    use crate::storage::disk::WAL_FILE_NAME;

    fn sql(engine: &mut DurableExecutor, sql: &str) -> Result<ResultSet> {
        engine.execute(&QueryParser::parse(sql)?, &QueryParams::default(), false)
    }

    fn seed(path: &str) -> DurableExecutor {
        let mut engine = DurableExecutor::new(path, false).unwrap();
        sql(&mut engine, "CREATE TABLE devices (id INT, geom GEOMETRY)").unwrap();
        sql(&mut engine, "CREATE TABLE zones (code TEXT, geom GEOMETRY)").unwrap();
        sql(
            &mut engine,
            "INSERT INTO zones VALUES ('A', ST_MakeEnvelope(0,0,10,10))",
        )
        .unwrap();
        sql(&mut engine, "CREATE CHANGEFEED edits ON devices").unwrap();
        sql(
            &mut engine,
            "CREATE GEOFENCE occupancy ON devices(geom) ZONES zones(geom)",
        )
        .unwrap();
        engine
    }

    fn insert(engine: &mut DurableExecutor, id: i64, bulk: bool) -> Result<usize> {
        engine.bulk_insert(
            "devices",
            &["id".into(), "geom".into()],
            vec![vec![Value::Integer(id), Value::Text("POINT(5 5)".into())]],
            bulk,
        )
    }

    #[test]
    fn failed_bulk_autocommit_has_no_rows_events_or_consumed_cursor() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().to_str().unwrap();
        {
            let mut engine = seed(path);
            let feeds = engine.changefeeds();
            let mut stream = feeds.subscribe("occupancy", None).unwrap();
            engine
                .disk
                .inject_wal_write_failure(&root.path().join(WAL_FILE_NAME));
            assert!(insert(&mut engine, 1, false).is_err());
            assert!(engine.get_tables()["devices"].rows.is_empty());
            assert!(engine.dirty.is_empty());
            assert!(feeds.changes("edits", None).unwrap().is_empty());
            assert_eq!(feeds.latest_sequence(), 0);
            assert!(stream.try_recv().unwrap().is_none());
            assert!(feeds.prepare_commit().unwrap().table().is_none());
        }
        let mut engine = DurableExecutor::new(path, false).unwrap();
        assert!(engine.get_tables()["devices"].rows.is_empty());
        insert(&mut engine, 2, false).unwrap();
        let event = engine
            .changefeeds()
            .changes("edits", None)
            .unwrap()
            .remove(0);
        assert_eq!((event.sequence, event.row_id), (1, 1));
        assert_eq!(event.operation, Operation::Insert);
    }

    #[test]
    fn failed_commit_retains_transaction_until_silent_rollback() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().to_str().unwrap();
        {
            let mut engine = seed(path);
            let feeds = engine.changefeeds();
            let mut stream = feeds.subscribe("edits", None).unwrap();
            sql(&mut engine, "BEGIN").unwrap();
            insert(&mut engine, 1, false).unwrap();
            engine
                .disk
                .inject_wal_write_failure(&root.path().join(WAL_FILE_NAME));
            assert!(sql(&mut engine, "COMMIT").is_err());
            assert!(engine.active);
            assert_eq!(engine.get_tables()["devices"].rows.len(), 1);
            assert!(stream.try_recv().unwrap().is_none());
            sql(&mut engine, "ROLLBACK").unwrap();
            assert!(!engine.active);
            assert!(engine.get_tables()["devices"].rows.is_empty());
            assert!(engine.dirty.is_empty());
            assert!(stream.try_recv().unwrap().is_none());
            assert!(feeds.prepare_commit().unwrap().table().is_none());
        }
        let engine = DurableExecutor::new(path, false).unwrap();
        assert!(engine.get_tables()["devices"].rows.is_empty());
        assert!(engine
            .changefeeds()
            .changes("edits", None)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn failed_autocommit_preserves_earlier_deferred_capture_only() {
        let root = tempfile::tempdir().unwrap();
        let mut engine = seed(root.path().to_str().unwrap());
        insert(&mut engine, 1, true).unwrap();
        engine
            .disk
            .inject_wal_write_failure(&root.path().join(WAL_FILE_NAME));
        assert!(insert(&mut engine, 2, false).is_err());
        assert_eq!(engine.get_tables()["devices"].rows.len(), 1);
        assert!(engine.dirty.contains("devices"));
        let feeds = engine.changefeeds();
        assert!(feeds.changes("edits", None).unwrap().is_empty());
        let prepared = feeds.prepare_commit().unwrap();
        let rows = &prepared.table().unwrap().rows;
        assert_eq!(rows.len(), 3); // metadata, insert 1, enter zone A
        for row in &rows[1..] {
            let Value::Blob(bytes) = &row.values[1] else {
                panic!("event payload");
            };
            let event: ChangeEvent = bincode::deserialize(bytes).unwrap();
            assert_eq!(event.row_id, 1);
            assert_eq!(
                event.new.as_ref().unwrap().get("id"),
                Some(&Value::Integer(1))
            );
        }
    }

    #[test]
    fn failed_sql_autocommit_restores_capture_and_indexes() {
        let root = tempfile::tempdir().unwrap();
        let mut engine = seed(root.path().to_str().unwrap());
        insert(&mut engine, 1, false).unwrap();
        sql(
            &mut engine,
            "CREATE INDEX devices_geom ON devices USING gist (geom)",
        )
        .unwrap();
        let feeds = engine.changefeeds();
        let cursor = feeds.latest_sequence();
        engine
            .disk
            .inject_wal_write_failure(&root.path().join(WAL_FILE_NAME));
        assert!(sql(&mut engine, "UPDATE devices SET geom = ST_Point(50,50)").is_err());
        let rows = sql(
            &mut engine,
            "SELECT id FROM devices WHERE ST_Covers(ST_MakeEnvelope(0,0,10,10),geom)",
        )
        .unwrap();
        assert_eq!(rows.rows.len(), 1);
        assert!(feeds.changes("edits", Some(cursor)).unwrap().is_empty());
        assert!(feeds.changes("occupancy", Some(cursor)).unwrap().is_empty());
        assert!(feeds.prepare_commit().unwrap().table().is_none());
    }
}
