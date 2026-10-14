use super::capture::{geofence_events, wkt_row, Pending, Snapshot};
use super::{error, Changefeeds, FeedKind, Journal, Shared, State};
use crate::storage::Table;
use crate::Result;
use std::collections::{BTreeSet, HashMap};
use std::sync::MutexGuard;
use std::time::{SystemTime, UNIX_EPOCH};

/// Restore only the failed statement, preserving any preceding deferred writes.
pub(crate) struct CaptureUndo {
    table: String,
    snapshot: Snapshot,
    pending: Option<Pending>,
}

/// Keeps the journal locked from preparation through the WAL commit. Retention
/// updates/subscriber attachment cannot race with a prepared version of the log.
/// Dropping this guard on failure never publishes or consumes pending changes.
pub(crate) struct PreparedCommit<'a> {
    shared: &'a Shared,
    state: MutexGuard<'a, State>,
    next: Option<Journal>,
    table: Option<Table>,
}

impl PreparedCommit<'_> {
    pub fn table(&self) -> Option<&Table> {
        self.table.as_ref()
    }

    /// Infallible publication, called only after the data and log share a
    /// successful WAL commit. No persistence or serialization occurs here.
    pub fn post_commit(mut self) {
        if let Some(next) = self.next.take() {
            self.state.journal = next;
            self.shared.changed.notify_all();
        }
        self.state.pending = None;
    }
}

impl Changefeeds {
    pub(crate) fn begin_transaction(&self) {
        self.shared.state.lock().unwrap().transaction_active = true;
    }

    pub(crate) fn end_transaction(&self) {
        self.shared.state.lock().unwrap().transaction_active = false;
    }

    pub(crate) fn rollback(&self) {
        let mut state = self.shared.state.lock().unwrap();
        if let Some(pending) = state.pending.take() {
            state.snapshots = pending.before;
        }
        state.transaction_active = false;
    }

    /// Capture successful DML before persistence. SQL and direct bulk insertion
    /// use this same path. UPDATE retains positions; other DML matches row IDs.
    pub(crate) fn capture_table(
        &self,
        name: &str,
        preserve_positions: bool,
        tables: &HashMap<String, Table>,
    ) -> Result<Option<CaptureUndo>> {
        let mut state = self.shared.state.lock().unwrap();
        let Some(snapshot) = state.snapshots.get(name) else {
            return Ok(None);
        };
        let before = snapshot.clone();
        let mut after = before.clone();
        let changes = after.capture(preserve_positions, &tables[name])?;
        if changes.is_empty() {
            return Ok(None);
        }
        let undo = CaptureUndo {
            table: name.into(),
            snapshot: before,
            pending: state.pending.clone(),
        };
        if state.pending.is_none() {
            state.pending = Some(Pending {
                before: state.snapshots.clone(),
                ..Pending::default()
            });
        }
        state.pending.as_mut().unwrap().record(changes);
        state.snapshots.insert(name.into(), after);
        Ok(Some(undo))
    }

    pub(crate) fn undo_capture(&self, undo: Option<CaptureUndo>) {
        if let Some(undo) = undo {
            let mut state = self.shared.state.lock().unwrap();
            state.snapshots.insert(undo.table, undo.snapshot);
            state.pending = undo.pending;
        }
    }

    pub(crate) fn prepare_commit(&self) -> Result<PreparedCommit<'_>> {
        let state = self.shared.state.lock().unwrap();
        let next = state
            .pending
            .as_ref()
            .map(|pending| prepare_journal(&state, pending))
            .transpose()?;
        let table = next.as_ref().map(Journal::table).transpose()?;
        Ok(PreparedCommit {
            shared: &self.shared,
            state,
            next,
            table,
        })
    }
}

fn prepare_journal(state: &State, pending: &Pending) -> Result<Journal> {
    let mut next = state.journal.clone();
    let mut events = Vec::new();
    for change in pending.changes.iter().filter(|c| c.old != c.new) {
        for (name, feed) in &next.metadata.feeds {
            if let FeedKind::Rows { tables } = &feed.kind {
                if tables.contains(&change.table) {
                    let mut event = change.clone();
                    event.feed = name.clone();
                    events.push(event);
                }
            }
        }
    }
    let touched: BTreeSet<_> = pending.changes.iter().map(|c| c.table.as_str()).collect();
    for (name, feed) in &next.metadata.feeds {
        if feed
            .kind
            .tables()
            .iter()
            .any(|table| touched.contains(table))
        {
            for mut event in geofence_events(&feed.kind, &pending.before, &state.snapshots) {
                event.feed = name.clone();
                events.push(event);
            }
        }
    }
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| error(e.to_string()))?
        .as_millis() as u64;
    for mut event in events {
        event.sequence = next.metadata.next_sequence;
        event.commit_sequence = next.metadata.next_commit;
        event.timestamp_ms = timestamp_ms;
        event.old = event.old.as_ref().map(wkt_row);
        event.new = event.new.as_ref().map(wkt_row);
        next.metadata.next_sequence += 1;
        next.events.push_back(event);
    }
    next.metadata.next_commit += 1;
    next.metadata.identities = state
        .snapshots
        .iter()
        .map(|(name, snapshot)| (name.clone(), snapshot.identities.clone()))
        .collect();
    next.trim();
    Ok(next)
}
