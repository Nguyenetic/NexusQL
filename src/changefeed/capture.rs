use super::{ChangeEvent, FeedKind, Operation, Zone};
use crate::storage::Table;
use crate::types::{Row, Value};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};

#[derive(Clone, Default, Serialize, Deserialize)]
pub(super) struct Identities {
    pub ids: Vec<u64>,
    pub next_id: u64,
}

#[derive(Clone)]
pub(super) struct Snapshot {
    pub identities: Identities,
    pub rows: Vec<Row>,
}

impl Snapshot {
    pub fn new(table: &Table) -> Self {
        Self {
            identities: Identities {
                ids: (1..=table.rows.len() as u64).collect(),
                next_id: table.rows.len() as u64 + 1,
            },
            rows: table.rows.clone(),
        }
    }

    /// UPDATE preserves vector positions. Other DML preserves unchanged rows;
    /// matching a multiset also handles INSERT OR REPLACE and duplicate rows.
    pub fn capture(&mut self, preserve_positions: bool, table: &Table) -> Result<Vec<ChangeEvent>> {
        let mut changes = Vec::new();
        if preserve_positions {
            for ((old, new), id) in self.rows.iter().zip(&table.rows).zip(&self.identities.ids) {
                if old != new {
                    changes.push(ChangeEvent::row(&table.name, *id, Some(old), Some(new)));
                }
            }
        } else {
            let mut by_value: HashMap<Vec<u8>, VecDeque<u64>> = HashMap::new();
            for (row, id) in self.rows.iter().zip(&self.identities.ids) {
                by_value
                    .entry(bincode::serialize(row)?)
                    .or_default()
                    .push_back(*id);
            }
            let mut new_ids = Vec::with_capacity(table.rows.len());
            let mut inserted = Vec::new();
            for row in &table.rows {
                let existing = by_value
                    .get_mut(&bincode::serialize(row)?)
                    .and_then(VecDeque::pop_front);
                let id = if let Some(id) = existing {
                    id
                } else {
                    let id = self.identities.next_id;
                    self.identities.next_id += 1;
                    inserted.push(ChangeEvent::row(&table.name, id, None, Some(row)));
                    id
                };
                new_ids.push(id);
            }
            let retained: std::collections::HashSet<_> = new_ids.iter().copied().collect();
            for (row, id) in self.rows.iter().zip(&self.identities.ids) {
                if !retained.contains(id) {
                    changes.push(ChangeEvent::row(&table.name, *id, Some(row), None));
                }
            }
            changes.extend(inserted);
            self.identities.ids = new_ids;
        }
        self.rows = table.rows.clone();
        Ok(changes)
    }

    fn row(&self, id: u64) -> Option<&Row> {
        self.identities
            .ids
            .iter()
            .position(|candidate| *candidate == id)
            .map(|index| &self.rows[index])
    }
}

#[derive(Clone, Default)]
pub(super) struct Pending {
    pub before: BTreeMap<String, Snapshot>,
    pub changes: Vec<ChangeEvent>,
    pub(super) positions: HashMap<(String, u64), usize>,
}

impl Pending {
    pub fn record(&mut self, changes: Vec<ChangeEvent>) {
        for change in changes {
            let key = (change.table.clone(), change.row_id);
            if let Some(index) = self.positions.get(&key) {
                self.changes[*index].new = change.new;
                self.changes[*index].operation = operation(
                    self.changes[*index].old.as_ref(),
                    self.changes[*index].new.as_ref(),
                );
            } else {
                self.positions.insert(key, self.changes.len());
                self.changes.push(change);
            }
        }
    }
}

pub(super) fn operation(old: Option<&Row>, new: Option<&Row>) -> Operation {
    match (old, new) {
        (None, _) => Operation::Insert,
        (_, None) => Operation::Delete,
        _ => Operation::Update,
    }
}

pub(super) fn wkt_row(row: &Row) -> Row {
    Row::new(
        row.columns.clone(),
        row.values
            .iter()
            .map(|v| match v {
                Value::Geometry(g) => Value::Text(g.as_text()),
                _ => v.clone(),
            })
            .collect(),
    )
}

type Memberships = BTreeMap<(u64, u64), ()>;

fn memberships(
    rows: &Snapshot,
    geometry: &str,
    zones: &Snapshot,
    zone_geometry: &str,
) -> Memberships {
    let mut inside = BTreeMap::new();
    for (row, id) in rows.rows.iter().zip(&rows.identities.ids) {
        for (zone, zone_id) in zones.rows.iter().zip(&zones.identities.ids) {
            let args = [
                zone.get(zone_geometry).cloned().unwrap_or(Value::Null),
                row.get(geometry).cloned().unwrap_or(Value::Null),
            ];
            if crate::geometry::evaluate("ST_COVERS", &args) == Value::Boolean(true) {
                inside.insert((*id, *zone_id), ());
            }
        }
    }
    inside
}

pub(super) fn geofence_events(
    kind: &FeedKind,
    before: &BTreeMap<String, Snapshot>,
    after: &BTreeMap<String, Snapshot>,
) -> Vec<ChangeEvent> {
    let FeedKind::Geofence {
        table,
        geometry,
        zones,
        zone_geometry,
    } = kind
    else {
        return Vec::new();
    };
    let old = memberships(&before[table], geometry, &before[zones], zone_geometry);
    let new = memberships(&after[table], geometry, &after[zones], zone_geometry);
    let mut events = Vec::new();
    // All exits precede all enters in a commit, including adjacent-zone moves.
    for (from, to, operation, zone_rows) in [
        (&old, &new, Operation::Exit, &before[zones]),
        (&new, &old, Operation::Enter, &after[zones]),
    ] {
        for (row_id, zone_id) in from.keys().filter(|key| !to.contains_key(key)) {
            let mut event = ChangeEvent::row(
                table,
                *row_id,
                before[table].row(*row_id),
                after[table].row(*row_id),
            );
            event.operation = operation;
            event.zone = Some(Zone {
                table: zones.clone(),
                row_id: *zone_id,
                values: wkt_row(zone_rows.row(*zone_id).expect("membership has a zone")),
            });
            events.push(event);
        }
    }
    events
}
