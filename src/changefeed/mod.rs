//! Commit-scoped row changes and boundary-inclusive geofences.
//!
//! SQLite's public-domain session extension is the design model: remember the
//! first before-image and final after-image of a row. No SQLite code is copied.

mod capture;
mod commit;
mod sql;
mod subscription;

pub use sql::parse_statement;
pub use subscription::ChangeSubscription;

use crate::query::ast::{DataType, Statement};
use crate::storage::{disk::DiskStorage, Table};
use crate::types::{Row, Value};
use crate::{NexusError, NexusSQL, Result};
use capture::{Identities, Pending, Snapshot};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex};

/// Reserved, versioned internal table. It is not exposed as a user table.
pub const LOG_TABLE: &str = "__nexus_changefeed_log";
pub const DEFAULT_RETENTION: usize = 10_000;

#[derive(Debug, Clone, PartialEq)]
pub enum ChangefeedStatement {
    Create { name: String, kind: FeedKind },
    Drop { name: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FeedKind {
    Rows {
        tables: Vec<String>,
    },
    Geofence {
        table: String,
        geometry: String,
        zones: String,
        zone_geometry: String,
    },
}

impl FeedKind {
    fn tables(&self) -> Vec<&str> {
        match self {
            Self::Rows { tables } => tables.iter().map(String::as_str).collect(),
            Self::Geofence { table, zones, .. } => vec![table, zones],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Insert,
    Update,
    Delete,
    Enter,
    Exit,
}

impl Operation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Insert => "insert",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Enter => "enter",
            Self::Exit => "exit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    pub table: String,
    pub row_id: u64,
    pub values: Row,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeEvent {
    pub feed: String,
    /// Global event cursor. `changes(feed, Some(n))` reads strictly after n.
    pub sequence: u64,
    /// All events from one commit share this number and timestamp.
    pub commit_sequence: u64,
    /// Milliseconds since the Unix epoch (UTC).
    pub timestamp_ms: u64,
    pub table: String,
    /// Stable synthetic identity, independent of shifting storage vector offsets.
    pub row_id: u64,
    pub operation: Operation,
    /// Complete row images, with geometry values converted to WKT text.
    pub old: Option<Row>,
    pub new: Option<Row>,
    pub zone: Option<Zone>,
}

impl ChangeEvent {
    fn row(table: &str, row_id: u64, old: Option<&Row>, new: Option<&Row>) -> Self {
        Self {
            feed: String::new(),
            sequence: 0,
            commit_sequence: 0,
            timestamp_ms: 0,
            table: table.into(),
            row_id,
            operation: capture::operation(old, new),
            old: old.cloned(),
            new: new.cloned(),
            zone: None,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Feed {
    kind: FeedKind,
    generation: u64,
    trimmed_through: u64,
}

#[derive(Clone, Serialize, Deserialize)]
struct Metadata {
    version: u32,
    retention: usize,
    next_sequence: u64,
    next_commit: u64,
    next_generation: u64,
    feeds: BTreeMap<String, Feed>,
    identities: BTreeMap<String, Identities>,
}

impl Default for Metadata {
    fn default() -> Self {
        Self {
            version: 1,
            retention: DEFAULT_RETENTION,
            next_sequence: 1,
            next_commit: 1,
            next_generation: 1,
            feeds: BTreeMap::new(),
            identities: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Default)]
struct Journal {
    metadata: Metadata,
    events: VecDeque<ChangeEvent>,
}

impl Journal {
    fn table(&self) -> Result<Table> {
        let mut table = Table::new(LOG_TABLE.into(), vec!["kind".into(), "payload".into()]);
        table.rows.push(Row::new(
            table.columns.clone(),
            vec![
                Value::Text("metadata-v1".into()),
                Value::Blob(bincode::serialize(&self.metadata)?),
            ],
        ));
        for event in &self.events {
            table.rows.push(Row::new(
                table.columns.clone(),
                vec![
                    Value::Text("event-v1".into()),
                    Value::Blob(bincode::serialize(event)?),
                ],
            ));
        }
        Ok(table)
    }

    fn persist(&self, disk: &DiskStorage) -> Result<()> {
        disk.commit_tables(&[&self.table()?], &[])
    }

    fn load(disk: &DiskStorage) -> Result<Self> {
        let Some(table) = disk.load_table(LOG_TABLE)? else {
            return Ok(Self::default());
        };
        let mut journal = Self::default();
        for (index, row) in table.rows.iter().enumerate() {
            match row.values.as_slice() {
                [Value::Text(kind), Value::Blob(bytes)] if index == 0 && kind == "metadata-v1" => {
                    journal.metadata = bincode::deserialize(bytes)?;
                }
                [Value::Text(kind), Value::Blob(bytes)] if index > 0 && kind == "event-v1" => {
                    journal.events.push_back(bincode::deserialize(bytes)?);
                }
                _ => return Err(error("Invalid changefeed log record")),
            }
        }
        if table.rows.is_empty() || journal.metadata.version != 1 {
            return Err(error("Unsupported changefeed log version"));
        }
        Ok(journal)
    }

    fn trim(&mut self) {
        while self.events.len() > self.metadata.retention {
            let event = self.events.pop_front().expect("nonempty log");
            if let Some(feed) = self.metadata.feeds.get_mut(&event.feed) {
                feed.trimmed_through = event.sequence;
            }
        }
    }

    fn validate_cursor(&self, name: &str, since: Option<u64>) -> Result<&Feed> {
        let feed = self
            .metadata
            .feeds
            .get(name)
            .ok_or_else(|| error(format!("Changefeed {name} does not exist")))?;
        if let Some(since) = since {
            if since < feed.trimmed_through {
                return Err(error(format!(
                    "Changefeed {name} cursor {since} expired; events through {} were trimmed; resnapshot before resuming",
                    feed.trimmed_through
                )));
            }
            if since >= self.metadata.next_sequence {
                return Err(error("Changefeed cursor is ahead of the latest sequence"));
            }
        }
        Ok(feed)
    }
}

struct State {
    journal: Journal,
    snapshots: BTreeMap<String, Snapshot>,
    pending: Option<Pending>,
    transaction_active: bool,
}

struct Shared {
    state: Mutex<State>,
    changed: Condvar,
    disk: Arc<DiskStorage>,
}

/// Thread-safe committed log handle. Clones share the same bounded log.
#[derive(Clone)]
pub struct Changefeeds {
    shared: Arc<Shared>,
}

fn error(message: impl Into<String>) -> NexusError {
    NexusError::QueryError(message.into())
}

impl Changefeeds {
    pub(crate) fn open(disk: Arc<DiskStorage>, tables: &HashMap<String, Table>) -> Result<Self> {
        let journal = Journal::load(&disk)?;
        let mut snapshots = BTreeMap::new();
        for (name, identities) in &journal.metadata.identities {
            let table = tables
                .get(name)
                .ok_or_else(|| error(format!("Watched table {name} is missing on reopen")))?;
            if identities.ids.len() != table.rows.len() {
                return Err(error(format!(
                    "Changefeed identities for {name} do not match recovered rows; resnapshot required"
                )));
            }
            snapshots.insert(
                name.clone(),
                Snapshot {
                    identities: identities.clone(),
                    rows: table.rows.clone(),
                },
            );
        }
        Ok(Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    journal,
                    snapshots,
                    pending: None,
                    transaction_active: false,
                }),
                changed: Condvar::new(),
                disk,
            }),
        })
    }

    pub fn changes(&self, feed: &str, since: Option<u64>) -> Result<Vec<ChangeEvent>> {
        let state = self.shared.state.lock().unwrap();
        state.journal.validate_cursor(feed, since)?;
        Ok(state
            .journal
            .events
            .iter()
            .filter(|e| e.feed == feed && since.is_none_or(|n| e.sequence > n))
            .cloned()
            .collect())
    }

    /// None starts at the current head; Some(n) atomically catches up then waits.
    pub fn subscribe(&self, feed: &str, since: Option<u64>) -> Result<ChangeSubscription> {
        let state = self.shared.state.lock().unwrap();
        let definition = state.journal.validate_cursor(feed, since)?;
        Ok(ChangeSubscription::new(
            self.clone(),
            feed.into(),
            definition.generation,
            since.unwrap_or(state.journal.metadata.next_sequence - 1),
        ))
    }

    /// Global bound in event rows. Trimming is immediate and persisted.
    pub fn set_retention(&self, max_events: usize) -> Result<()> {
        if max_events == 0 {
            return Err(error("Changefeed retention must be at least one event"));
        }
        let mut state = self.shared.state.lock().unwrap();
        if state.transaction_active {
            return Err(error(
                "Changefeed retention cannot change inside a transaction",
            ));
        }
        let mut next = state.journal.clone();
        next.metadata.retention = max_events;
        next.trim();
        next.persist(&self.shared.disk)?;
        state.journal = next;
        self.shared.changed.notify_all();
        Ok(())
    }

    pub fn retention(&self) -> usize {
        self.shared.state.lock().unwrap().journal.metadata.retention
    }

    pub fn latest_sequence(&self) -> u64 {
        self.shared
            .state
            .lock()
            .unwrap()
            .journal
            .metadata
            .next_sequence
            - 1
    }

    pub(crate) fn enabled(&self) -> bool {
        !self
            .shared
            .state
            .lock()
            .unwrap()
            .journal
            .metadata
            .feeds
            .is_empty()
    }

    /// Reject schema changes to watched tables and writes to the internal log.
    pub(crate) fn validate_statement(&self, statement: &Statement) -> Result<()> {
        let state = self.shared.state.lock().unwrap();
        if let Statement::CreateTable { name, .. } | Statement::DropTable { name, .. } = statement {
            if name == LOG_TABLE || state.snapshots.contains_key(name) {
                return Err(error(
                    "Drop the changefeeds before changing a watched table's schema",
                ));
            }
        }
        let table = match statement {
            Statement::Insert { table, .. }
            | Statement::Update { table, .. }
            | Statement::CreateIndex { table, .. } => Some(table),
            Statement::Delete { from, .. } => Some(from),
            _ => None,
        };
        if table.is_some_and(|table| table == LOG_TABLE) {
            return Err(error("The internal changefeed log is read-only"));
        }
        Ok(())
    }

    pub(crate) fn manage(
        &self,
        command: &ChangefeedStatement,
        tables: &HashMap<String, Table>,
        in_transaction: bool,
    ) -> Result<()> {
        if in_transaction {
            return Err(error(
                "CREATE/DROP CHANGEFEED and CREATE GEOFENCE require autocommit",
            ));
        }
        let mut state = self.shared.state.lock().unwrap();
        let mut next = state.journal.clone();
        let mut snapshots = state.snapshots.clone();
        match command {
            ChangefeedStatement::Create { name, kind } => {
                if next.metadata.feeds.contains_key(name) {
                    return Err(error(format!("Changefeed {name} already exists")));
                }
                for table_name in kind.tables() {
                    let table = tables
                        .get(table_name)
                        .filter(|_| table_name != LOG_TABLE)
                        .ok_or_else(|| error(format!("Table {table_name} not found")))?;
                    snapshots
                        .entry(table_name.into())
                        .or_insert_with(|| Snapshot::new(table));
                }
                if let FeedKind::Geofence {
                    table,
                    geometry,
                    zones,
                    zone_geometry,
                } = kind
                {
                    for (name, column) in [(table, geometry), (zones, zone_geometry)] {
                        if tables[name].schema.column_types.get(column) != Some(&DataType::Geometry)
                        {
                            return Err(error(format!(
                                "{name}.{column} must be a GEOMETRY column"
                            )));
                        }
                    }
                }
                next.metadata.feeds.insert(
                    name.clone(),
                    Feed {
                        kind: kind.clone(),
                        generation: next.metadata.next_generation,
                        trimmed_through: 0,
                    },
                );
                next.metadata.next_generation += 1;
            }
            ChangefeedStatement::Drop { name } => {
                if next.metadata.feeds.remove(name).is_none() {
                    return Err(error(format!("Changefeed {name} does not exist")));
                }
                next.events.retain(|event| &event.feed != name);
                let watched: BTreeSet<_> = next
                    .metadata
                    .feeds
                    .values()
                    .flat_map(|feed| feed.kind.tables())
                    .collect();
                snapshots.retain(|name, _| watched.contains(name.as_str()));
            }
        }
        next.metadata.identities = snapshots
            .iter()
            .map(|(name, snapshot)| (name.clone(), snapshot.identities.clone()))
            .collect();
        // DurableExecutor flushes preceding deferred writes before feed DDL.
        next.persist(&self.shared.disk)?;
        state.journal = next;
        state.snapshots = snapshots;
        self.shared.changed.notify_all();
        Ok(())
    }
}

impl NexusSQL {
    pub fn changes(&self, feed: &str, since: Option<u64>) -> Result<Vec<ChangeEvent>> {
        self.changefeeds.changes(feed, since)
    }

    pub fn subscribe(&self, feed: &str, since: Option<u64>) -> Result<ChangeSubscription> {
        self.changefeeds.subscribe(feed, since)
    }

    pub fn set_changefeed_retention(&self, max_events: usize) -> Result<()> {
        self.changefeeds.set_retention(max_events)
    }

    pub fn changefeeds(&self) -> &Changefeeds {
        &self.changefeeds
    }
}
