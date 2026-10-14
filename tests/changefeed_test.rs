use nexusql::changefeed::{ChangeEvent, Operation};
use nexusql::{NexusSQL, QueryParser, Value};
use std::time::{Duration, Instant};

async fn memory() -> NexusSQL {
    let db = NexusSQL::new(":memory:").await.unwrap();
    db.execute("CREATE TABLE devices (id INT PRIMARY KEY, name TEXT, geom GEOMETRY)")
        .await
        .unwrap();
    db.execute("CREATE CHANGEFEED devices_feed ON devices")
        .await
        .unwrap();
    db
}

fn operations(events: &[ChangeEvent]) -> Vec<Operation> {
    events.iter().map(|e| e.operation).collect()
}

fn value<'a>(event: &'a ChangeEvent, column: &str) -> &'a Value {
    event
        .new
        .as_ref()
        .or(event.old.as_ref())
        .unwrap()
        .get(column)
        .unwrap()
}

async fn fence() -> NexusSQL {
    let db = memory().await;
    db.execute("CREATE TABLE zones (code TEXT, geom GEOMETRY)")
        .await
        .unwrap();
    db.execute("INSERT INTO zones VALUES ('A', ST_MakeEnvelope(0,0,10,10)), ('B', ST_MakeEnvelope(10,0,20,10))")
        .await.unwrap();
    db.execute("CREATE GEOFENCE occupancy ON devices(geom) ZONES zones(geom)")
        .await
        .unwrap();
    db
}

fn zone(event: &ChangeEvent) -> &Value {
    event.zone.as_ref().unwrap().values.get("code").unwrap()
}

#[test]
fn parser_handles_comments_quotes_and_rejects_incomplete_or_extra_sql() {
    for sql in [
        "CREATE CHANGEFEED changes ON devices, cameras;",
        "-- map\nCREATE /* live */ CHANGEFEED \"map feed\" ON \"map devices\";",
        "create geofence occupancy on devices(geom) zones zones(geom)",
        "DROP CHANGEFEED occupancy; -- done",
    ] {
        assert!(QueryParser::parse(sql).is_ok(), "{sql}");
    }
    for sql in [
        "CREATE CHANGEFEED f",
        "CREATE CHANGEFEED f ON",
        "CREATE CHANGEFEED f ON devices,",
        "DROP CHANGEFEED",
        "CREATE GEOFENCE f ON devices ZONES zones(geom)",
        "CREATE GEOFENCE f ON devices(geom) ZONES zones()",
        "CREATE GEOFENCE f ON devices(geom, name) ZONES zones(geom)",
        "CREATE CHANGEFEED f ON devices; DROP TABLE devices",
    ] {
        assert!(QueryParser::parse(sql).is_err(), "{sql}");
    }
    assert!(QueryParser::parse("SELECT 'CREATE CHANGEFEED f ON devices'").is_ok());
}

#[tokio::test]
async fn insert_update_delete_exact_order_images_and_stable_identity() {
    let db = memory().await;
    db.execute(
        "INSERT INTO devices VALUES (1, 'camera', ST_Point(1,2)), (2, 'press', ST_Point(3,4))",
    )
    .await
    .unwrap();
    db.execute("UPDATE devices SET geom = ST_Point(5,6) WHERE id = 1")
        .await
        .unwrap();
    db.execute("DELETE FROM devices WHERE id = 1")
        .await
        .unwrap();
    db.execute("UPDATE devices SET name = 'press-2' WHERE id = 2")
        .await
        .unwrap();
    let events = db.changes("devices_feed", None).unwrap();
    assert_eq!(
        operations(&events),
        vec![
            Operation::Insert,
            Operation::Insert,
            Operation::Update,
            Operation::Delete,
            Operation::Update
        ]
    );
    assert_eq!(
        events.iter().map(|e| e.sequence).collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5]
    );
    assert_eq!(
        events.iter().map(|e| e.row_id).collect::<Vec<_>>(),
        vec![1, 2, 1, 1, 2]
    );
    assert_eq!(
        events.iter().map(|e| e.commit_sequence).collect::<Vec<_>>(),
        vec![1, 1, 2, 3, 4]
    );
    assert!(events[0].old.is_none());
    assert!(events[3].new.is_none());
    assert_eq!(
        events[2].old.as_ref().unwrap().get("geom"),
        Some(&Value::Text("POINT(1 2)".into()))
    );
    assert_eq!(value(&events[2], "geom"), &Value::Text("POINT(5 6)".into()));
    assert_eq!(events[0].timestamp_ms, events[1].timestamp_ms);
    assert!(events
        .iter()
        .all(|e| e.timestamp_ms > 0 && e.table == "devices"));
}

#[tokio::test]
async fn commit_coalesces_rows_in_first_touch_order() {
    let db = memory().await;
    db.execute("INSERT INTO devices VALUES (1, 'one', NULL), (2, 'two', NULL)")
        .await
        .unwrap();
    let cursor = db.changefeeds().latest_sequence();
    db.execute("BEGIN").await.unwrap();
    db.execute("UPDATE devices SET name = 'second' WHERE id = 2")
        .await
        .unwrap();
    db.execute("UPDATE devices SET name = 'first' WHERE id = 1")
        .await
        .unwrap();
    db.execute("UPDATE devices SET name = 'second-final' WHERE id = 2")
        .await
        .unwrap();
    db.execute("INSERT INTO devices VALUES (3, 'transient', NULL)")
        .await
        .unwrap();
    db.execute("DELETE FROM devices WHERE id = 3")
        .await
        .unwrap();
    assert!(db.changes("devices_feed", Some(cursor)).unwrap().is_empty());
    db.execute("COMMIT").await.unwrap();
    let events = db.changes("devices_feed", Some(cursor)).unwrap();
    assert_eq!(
        operations(&events),
        vec![Operation::Update, Operation::Update]
    );
    assert_eq!(
        events.iter().map(|e| e.row_id).collect::<Vec<_>>(),
        vec![2, 1]
    );
    assert_eq!(events[0].commit_sequence, events[1].commit_sequence);
    assert_eq!(events[0].timestamp_ms, events[1].timestamp_ms);
    assert_eq!(
        events[0].old.as_ref().unwrap().get("name"),
        Some(&Value::Text("two".into()))
    );
    assert_eq!(
        value(&events[0], "name"),
        &Value::Text("second-final".into())
    );
}

#[tokio::test]
async fn rollback_and_noop_emit_nothing_to_log_or_subscriber() {
    let db = memory().await;
    let mut subscriber = db.subscribe("devices_feed", None).unwrap();
    db.execute("BEGIN").await.unwrap();
    db.execute("INSERT INTO devices VALUES (1, 'gone', NULL)")
        .await
        .unwrap();
    assert!(subscriber.try_recv().unwrap().is_none());
    db.execute("ROLLBACK").await.unwrap();
    assert!(db.changes("devices_feed", None).unwrap().is_empty());
    assert!(subscriber.try_recv().unwrap().is_none());
    db.execute("INSERT INTO devices VALUES (1, 'kept', NULL)")
        .await
        .unwrap();
    let event = subscriber.recv().unwrap();
    assert_eq!(event.row_id, 1);
    db.execute("BEGIN").await.unwrap();
    db.execute("UPDATE devices SET name = 'temporary'")
        .await
        .unwrap();
    db.execute("UPDATE devices SET name = 'kept'")
        .await
        .unwrap();
    db.execute("COMMIT").await.unwrap();
    db.execute("UPDATE devices SET name = name").await.unwrap();
    assert!(subscriber.try_recv().unwrap().is_none());
    assert_eq!(db.changes("devices_feed", None).unwrap().len(), 1);
}

#[tokio::test]
async fn catchup_is_exclusive_and_subscribe_continues_without_duplicates() {
    let db = memory().await;
    db.execute("INSERT INTO devices VALUES (1, 'one', NULL), (2, 'two', NULL)")
        .await
        .unwrap();
    let mut subscription = db.subscribe("devices_feed", Some(1)).unwrap();
    assert_eq!(subscription.recv().unwrap().sequence, 2);
    assert!(subscription.try_recv().unwrap().is_none());
    db.execute("DELETE FROM devices WHERE id = 1")
        .await
        .unwrap();
    assert_eq!(subscription.recv().unwrap().sequence, 3);
    assert_eq!(db.changes("devices_feed", Some(1)).unwrap().len(), 2);
    assert!(db.changes("devices_feed", Some(99)).is_err());
}

#[tokio::test]
async fn blocking_subscription_wakes_on_commit() {
    let db = memory().await;
    let mut subscription = db.subscribe("devices_feed", None).unwrap();
    let reader = std::thread::spawn(move || {
        subscription
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap()
    });
    db.execute("BEGIN").await.unwrap();
    db.execute("INSERT INTO devices VALUES (1, 'one', NULL)")
        .await
        .unwrap();
    db.execute("COMMIT").await.unwrap();
    assert_eq!(reader.join().unwrap().operation, Operation::Insert);
}

#[tokio::test]
async fn retention_is_bounded_and_reports_expired_cursors() {
    let db = memory().await;
    db.set_changefeed_retention(2).unwrap();
    let mut slow = db.subscribe("devices_feed", None).unwrap();
    db.execute("INSERT INTO devices VALUES (1, 'one', NULL), (2, 'two', NULL), (3, 'three', NULL)")
        .await
        .unwrap();
    let retained = db.changes("devices_feed", None).unwrap();
    assert_eq!(
        retained.iter().map(|e| e.sequence).collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert!(db
        .changes("devices_feed", Some(0))
        .unwrap_err()
        .to_string()
        .contains("expired"));
    assert_eq!(db.changes("devices_feed", Some(1)).unwrap().len(), 2);
    assert!(slow.try_recv().unwrap_err().to_string().contains("expired"));
    db.set_changefeed_retention(1).unwrap();
    assert_eq!(db.changes("devices_feed", None).unwrap().len(), 1);
    assert!(db.set_changefeed_retention(0).is_err());
}

#[tokio::test]
async fn multiple_tables_and_feeds_are_filtered_without_false_gap_errors() {
    let db = memory().await;
    db.execute("CREATE TABLE cameras (id INT, device INT)")
        .await
        .unwrap();
    db.execute("CREATE CHANGEFEED kfm ON devices, cameras")
        .await
        .unwrap();
    db.execute("CREATE CHANGEFEED cameras_feed ON cameras")
        .await
        .unwrap();
    db.set_changefeed_retention(3).unwrap();
    db.execute("BEGIN").await.unwrap();
    db.execute("INSERT INTO devices VALUES (1, 'camera', NULL)")
        .await
        .unwrap();
    db.execute("INSERT INTO cameras VALUES (10, 1)")
        .await
        .unwrap();
    db.execute("COMMIT").await.unwrap();
    let events = db.changes("kfm", None).unwrap();
    assert_eq!(
        events.iter().map(|e| e.table.as_str()).collect::<Vec<_>>(),
        vec!["devices", "cameras"]
    );
    assert_eq!(events[0].commit_sequence, events[1].commit_sequence);
    assert_eq!(db.changes("cameras_feed", Some(0)).unwrap().len(), 1);
    assert!(db.changes("devices_feed", Some(0)).is_err());
}

#[tokio::test]
async fn reopen_preserves_events_definitions_cursor_retention_and_row_ids() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_str().unwrap();
    let cursor;
    {
        let db = NexusSQL::new(path).await.unwrap();
        db.execute("CREATE TABLE devices (id INT, name TEXT)")
            .await
            .unwrap();
        db.execute("CREATE CHANGEFEED f ON devices").await.unwrap();
        db.set_changefeed_retention(2).unwrap();
        db.execute("INSERT INTO devices VALUES (1, 'one'), (2, 'two'), (3, 'three')")
            .await
            .unwrap();
        db.execute("DELETE FROM devices WHERE id = 1")
            .await
            .unwrap();
        cursor = db.changefeeds().latest_sequence();
        db.execute("BEGIN").await.unwrap();
        db.execute("INSERT INTO devices VALUES (4, 'rollback')")
            .await
            .unwrap();
        db.execute("ROLLBACK").await.unwrap();
        db.shutdown().await.unwrap();
    }
    let db = NexusSQL::new(path).await.unwrap();
    assert_eq!(db.changefeeds().retention(), 2);
    assert_eq!(db.changes("f", None).unwrap().len(), 2);
    assert!(db.changes("f", Some(0)).is_err());
    assert!(db.changes("f", Some(cursor)).unwrap().is_empty());
    db.execute("UPDATE devices SET name = 'after reopen' WHERE id = 2")
        .await
        .unwrap();
    let events = db.changes("f", Some(cursor)).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].row_id, 2);
    assert_eq!(events[0].sequence, cursor + 1);
    db.shutdown().await.unwrap();
}

#[tokio::test]
async fn fast_and_direct_bulk_inserts_share_the_commit_hook() {
    let db = memory().await;
    db.execute("INSERT INTO devices VALUES (1, 'fast', 'POINT(1 1)')")
        .await
        .unwrap();
    db.execute("BEGIN").await.unwrap();
    db.bulk_insert(
        "devices",
        &["id".into(), "name".into(), "geom".into()],
        vec![
            Value::Integer(2),
            Value::Text("direct".into()),
            Value::Text("POINT(2 2)".into()),
        ],
        1,
    )
    .await
    .unwrap();
    assert_eq!(db.changes("devices_feed", None).unwrap().len(), 1);
    db.execute("COMMIT").await.unwrap();
    assert_eq!(db.changes("devices_feed", None).unwrap().len(), 2);
}

#[tokio::test]
async fn invalid_statement_has_no_partial_feed_or_row_mutation() {
    let db = memory().await;
    assert!(db
        .execute("INSERT INTO devices VALUES (1, 'ok', 'POINT(1 1)'), (2, 'bad', 'not geometry')")
        .await
        .is_err());
    assert!(db.changes("devices_feed", None).unwrap().is_empty());
    assert!(db
        .execute("SELECT * FROM devices")
        .await
        .unwrap()
        .rows
        .is_empty());
    db.execute("INSERT INTO devices VALUES (3, 'valid', NULL)")
        .await
        .unwrap();
    assert_eq!(db.changes("devices_feed", None).unwrap().len(), 1);
}

#[tokio::test]
async fn management_validates_schema_transaction_and_drop_lifecycle() {
    let db = memory().await;
    assert!(db.execute("CREATE CHANGEFEED bad ON absent").await.is_err());
    assert!(db
        .execute("CREATE CHANGEFEED devices_feed ON devices")
        .await
        .is_err());
    assert!(db
        .execute("CREATE GEOFENCE bad ON devices(name) ZONES devices(geom)")
        .await
        .is_err());
    assert!(db.execute("DROP TABLE devices").await.is_err());
    db.execute("BEGIN").await.unwrap();
    assert!(db.execute("CREATE CHANGEFEED no ON devices").await.is_err());
    assert!(db.execute("DROP CHANGEFEED devices_feed").await.is_err());
    assert!(db.set_changefeed_retention(4).is_err());
    db.execute("ROLLBACK").await.unwrap();
    let mut subscriber = db.subscribe("devices_feed", None).unwrap();
    db.execute("DROP CHANGEFEED devices_feed").await.unwrap();
    db.execute("CREATE CHANGEFEED devices_feed ON devices")
        .await
        .unwrap();
    assert!(subscriber.try_recv().is_err());
    db.execute("DROP CHANGEFEED devices_feed").await.unwrap();
    db.execute("DROP TABLE devices").await.unwrap();
}

#[tokio::test]
async fn geofence_boundary_is_inside_both_adjacent_zones() {
    let db = fence().await;
    db.execute("INSERT INTO devices VALUES (1, 'on wall', ST_Point(10,5))")
        .await
        .unwrap();
    let events = db.changes("occupancy", None).unwrap();
    assert_eq!(
        operations(&events),
        vec![Operation::Enter, Operation::Enter]
    );
    assert_eq!(zone(&events[0]), &Value::Text("A".into()));
    assert_eq!(zone(&events[1]), &Value::Text("B".into()));
    db.execute("DELETE FROM devices WHERE id = 1")
        .await
        .unwrap();
    let events = db.changes("occupancy", Some(events[1].sequence)).unwrap();
    assert_eq!(operations(&events), vec![Operation::Exit, Operation::Exit]);
}

#[tokio::test]
async fn adjacent_move_exits_a_then_enters_b_in_one_commit() {
    let db = fence().await;
    db.execute("INSERT INTO devices VALUES (1, 'moving', ST_Point(5,5))")
        .await
        .unwrap();
    let cursor = db.changefeeds().latest_sequence();
    db.execute("BEGIN").await.unwrap();
    db.execute("UPDATE devices SET geom = ST_Point(10,5)")
        .await
        .unwrap();
    db.execute("UPDATE devices SET geom = ST_Point(15,5)")
        .await
        .unwrap();
    assert!(db.changes("occupancy", Some(cursor)).unwrap().is_empty());
    db.execute("COMMIT").await.unwrap();
    let events = db.changes("occupancy", Some(cursor)).unwrap();
    assert_eq!(operations(&events), vec![Operation::Exit, Operation::Enter]);
    assert_eq!(zone(&events[0]), &Value::Text("A".into()));
    assert_eq!(zone(&events[1]), &Value::Text("B".into()));
    assert_eq!(events[0].commit_sequence, events[1].commit_sequence);
    assert_eq!(events[0].timestamp_ms, events[1].timestamp_ms);
    assert_eq!(events[0].row_id, events[1].row_id);
}

#[tokio::test]
async fn zone_edits_inserts_and_deletes_reevaluate_stationary_rows() {
    let db = fence().await;
    db.execute(
        "INSERT INTO devices VALUES (1, 'stationary', ST_Point(5,5)), (2, 'far', ST_Point(50,50))",
    )
    .await
    .unwrap();
    let cursor = db.changefeeds().latest_sequence();
    db.execute("UPDATE zones SET geom = ST_MakeEnvelope(40,40,60,60) WHERE code = 'A'")
        .await
        .unwrap();
    let events = db.changes("occupancy", Some(cursor)).unwrap();
    assert_eq!(operations(&events), vec![Operation::Exit, Operation::Enter]);
    assert_eq!(
        events.iter().map(|e| e.row_id).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(events[0].old, events[0].new);
    let cursor = db.changefeeds().latest_sequence();
    db.execute("DELETE FROM zones WHERE code = 'A'")
        .await
        .unwrap();
    db.execute("INSERT INTO zones VALUES ('C', ST_MakeEnvelope(0,0,10,10))")
        .await
        .unwrap();
    assert_eq!(
        operations(&db.changes("occupancy", Some(cursor)).unwrap()),
        vec![Operation::Exit, Operation::Enter]
    );
}

#[tokio::test]
async fn zone_and_device_edits_use_final_commit_state_and_rollback_is_silent() {
    let db = fence().await;
    db.execute("INSERT INTO devices VALUES (1, 'one', ST_Point(5,5))")
        .await
        .unwrap();
    let cursor = db.changefeeds().latest_sequence();
    db.execute("BEGIN").await.unwrap();
    db.execute("UPDATE zones SET geom = ST_MakeEnvelope(30,30,40,40) WHERE code = 'A'")
        .await
        .unwrap();
    db.execute("UPDATE devices SET geom = ST_Point(35,35)")
        .await
        .unwrap();
    db.execute("COMMIT").await.unwrap();
    assert!(db.changes("occupancy", Some(cursor)).unwrap().is_empty());
    db.execute("BEGIN").await.unwrap();
    db.execute("DELETE FROM zones").await.unwrap();
    db.execute("UPDATE devices SET geom = NULL").await.unwrap();
    db.execute("ROLLBACK").await.unwrap();
    assert!(db.changes("occupancy", Some(cursor)).unwrap().is_empty());
}

#[tokio::test]
async fn geofence_creation_has_no_synthetic_backfill_and_null_leaves_zone() {
    let db = memory().await;
    db.execute("CREATE TABLE zones (code TEXT, geom GEOMETRY)")
        .await
        .unwrap();
    db.execute("INSERT INTO zones VALUES ('A', ST_MakeEnvelope(0,0,10,10))")
        .await
        .unwrap();
    db.execute("INSERT INTO devices VALUES (1, 'existing', ST_Point(5,5))")
        .await
        .unwrap();
    db.execute("CREATE GEOFENCE f ON devices(geom) ZONES zones(geom)")
        .await
        .unwrap();
    assert!(db.changes("f", None).unwrap().is_empty());
    db.execute("UPDATE devices SET geom = NULL").await.unwrap();
    assert_eq!(
        operations(&db.changes("f", None).unwrap()),
        vec![Operation::Exit]
    );
    db.execute("DROP CHANGEFEED f").await.unwrap();
}

#[tokio::test]
async fn geofence_holes_and_srid_mismatch_follow_st_covers() {
    let db = memory().await;
    db.execute("CREATE TABLE zones (code TEXT, geom GEOMETRY)")
        .await
        .unwrap();
    db.execute("INSERT INTO zones VALUES ('hole', ST_GeomFromText('POLYGON((0 0,10 0,10 10,0 10,0 0),(3 3,7 3,7 7,3 7,3 3))', 32616))").await.unwrap();
    db.execute("CREATE GEOFENCE f ON devices(geom) ZONES zones(geom)")
        .await
        .unwrap();
    db.execute("INSERT INTO devices VALUES (1, 'hole interior', ST_GeomFromText('POINT(5 5)',32616)), (2, 'hole edge', ST_GeomFromText('POINT(3 5)',32616)), (3, 'wrong srid', ST_Point(1,1))").await.unwrap();
    let events = db.changes("f", None).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(value(&events[0], "id"), &Value::Integer(2));
}

#[tokio::test]
async fn duplicate_rows_keep_independent_stable_row_ids() {
    let db = memory().await;
    db.execute(
        "INSERT INTO devices VALUES (1, 'same', NULL), (1, 'same', NULL), (2, 'other', NULL)",
    )
    .await
    .unwrap();
    db.execute("DELETE FROM devices WHERE id = 1")
        .await
        .unwrap();
    db.execute("UPDATE devices SET id = 9").await.unwrap();
    let events = db.changes("devices_feed", None).unwrap();
    assert_eq!(
        events.iter().map(|e| e.row_id).collect::<Vec<_>>(),
        vec![1, 2, 3, 1, 2, 3]
    );
    assert_eq!(events[5].operation, Operation::Update);
}

#[tokio::test]
async fn measured_kfm_batch_has_exact_counts_and_a_bounded_log() {
    let db = fence().await;
    db.set_changefeed_retention(2000).unwrap();
    let rows = (1..=1000)
        .flat_map(|id| {
            [
                Value::Integer(id),
                Value::Text(format!("CAM-{id}")),
                Value::Text("POINT(5 5)".into()),
            ]
        })
        .collect();
    let start = Instant::now();
    db.bulk_insert(
        "devices",
        &["id".into(), "name".into(), "geom".into()],
        rows,
        1000,
    )
    .await
    .unwrap();
    let elapsed = start.elapsed();
    let row_events = db.changes("devices_feed", None).unwrap();
    let fence_events = db.changes("occupancy", None).unwrap();
    assert_eq!(row_events.len(), 1000);
    assert_eq!(fence_events.len(), 1000);
    eprintln!(
        "KFM measurement: 1000 devices x 2 zones, 2000 events, in-memory debug commit {:.3} ms",
        elapsed.as_secs_f64() * 1000.0
    );
}

#[tokio::test]
async fn bulk_mode_publishes_only_at_flush_or_commit_and_flush_never_commits_begin() {
    let root = tempfile::tempdir().unwrap();
    for path in [
        ":memory:".to_string(),
        root.path().to_str().unwrap().to_string(),
    ] {
        let db = NexusSQL::new(&path).await.unwrap();
        db.execute("CREATE TABLE devices (id INT, geom GEOMETRY)")
            .await
            .unwrap();
        db.execute("CREATE CHANGEFEED f ON devices").await.unwrap();
        let mut stream = db.subscribe("f", None).unwrap();
        db.set_bulk_mode(true).await;
        db.execute("INSERT INTO devices VALUES (1, 'POINT(1 1)')")
            .await
            .unwrap();
        assert!(stream.try_recv().unwrap().is_none());
        assert_eq!(db.changefeeds().latest_sequence(), 0);
        db.flush().await.unwrap();
        assert_eq!(stream.recv().unwrap().operation, Operation::Insert);
        db.execute("BEGIN").await.unwrap();
        db.execute("UPDATE devices SET geom = ST_Point(2,2)")
            .await
            .unwrap();
        db.flush().await.unwrap();
        assert!(stream.try_recv().unwrap().is_none());
        db.execute("ROLLBACK").await.unwrap();
        db.flush().await.unwrap();
        assert!(stream.try_recv().unwrap().is_none());
        db.execute("BEGIN").await.unwrap();
        db.execute("UPDATE devices SET geom = ST_Point(3,3)")
            .await
            .unwrap();
        db.execute("COMMIT").await.unwrap();
        assert_eq!(stream.recv().unwrap().operation, Operation::Update);
        assert_eq!(db.changes("f", None).unwrap().len(), 2);
        drop(stream);
        db.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn failed_bulk_validation_does_not_leak_into_later_commit_on_disk() {
    let root = tempfile::tempdir().unwrap();
    let db = NexusSQL::new(root.path().to_str().unwrap()).await.unwrap();
    db.execute("CREATE TABLE devices (id INT, geom GEOMETRY)")
        .await
        .unwrap();
    db.execute("CREATE CHANGEFEED f ON devices").await.unwrap();
    let mut stream = db.subscribe("f", None).unwrap();
    let columns = vec!["id".into(), "geom".into()];
    for transactional in [false, true] {
        if transactional {
            db.execute("BEGIN").await.unwrap();
        }
        assert!(db
            .bulk_insert(
                "devices",
                &columns,
                vec![
                    Value::Integer(1),
                    Value::Text("POINT(1 1)".into()),
                    Value::Integer(2),
                    Value::Text("invalid geometry".into()),
                ],
                2
            )
            .await
            .is_err());
        assert!(stream.try_recv().unwrap().is_none());
        assert!(db
            .execute("SELECT * FROM devices")
            .await
            .unwrap()
            .rows
            .is_empty());
        if transactional {
            db.execute("ROLLBACK").await.unwrap();
        }
    }
    db.bulk_insert(
        "devices",
        &columns,
        vec![Value::Integer(3), Value::Text("POINT(3 3)".into())],
        1,
    )
    .await
    .unwrap();
    let event = stream.recv().unwrap();
    assert_eq!((event.sequence, event.row_id), (1, 1));
    assert_eq!(value(&event, "id"), &Value::Integer(3));
    drop(stream);
    db.shutdown().await.unwrap();
    let db = NexusSQL::new(root.path().to_str().unwrap()).await.unwrap();
    assert_eq!(db.changes("f", None).unwrap(), vec![event]);
    db.shutdown().await.unwrap();
}

// Child exits without Drop/checkpoint, leaving only the acknowledged WAL prefix.
#[test]
fn changefeed_wal_child() {
    let Ok(path) = std::env::var("NEXUSQL_CHANGEFEED_WAL_CHILD") else {
        return;
    };
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let db = NexusSQL::new(&path).await.unwrap();
        db.execute("PRAGMA nexusql_wal_checkpoint_bytes = 0").await.unwrap();
        db.execute("CREATE TABLE devices (id INT, geom GEOMETRY)").await.unwrap();
        db.execute("CREATE TABLE cameras (id INT, device_id INT)").await.unwrap();
        db.execute("CREATE TABLE zones (code TEXT, geom GEOMETRY)").await.unwrap();
        db.execute("INSERT INTO zones VALUES ('A', ST_MakeEnvelope(0,0,10,10)), ('B', ST_MakeEnvelope(10,0,20,10))").await.unwrap();
        db.execute("CREATE CHANGEFEED f ON devices, cameras").await.unwrap();
        db.execute("CREATE GEOFENCE occupancy ON devices(geom) ZONES zones(geom)").await.unwrap();
        db.execute("INSERT INTO devices VALUES (1, 'POINT(5 5)')").await.unwrap();
        let cursor = db.changefeeds().latest_sequence();
        let wal_path = std::path::Path::new(&path).join("default").join(nexusql::storage::disk::WAL_FILE_NAME);
        let prefix = std::fs::metadata(wal_path).unwrap().len();
        db.execute("BEGIN").await.unwrap();
        db.execute("UPDATE devices SET geom = ST_Point(15,5)").await.unwrap();
        db.execute("INSERT INTO cameras VALUES (101, 1)").await.unwrap();
        db.execute("COMMIT").await.unwrap();
        let committed = db.changefeeds().latest_sequence();
        // These direct bulk and zone edits must never enter the WAL or feed log.
        db.execute("BEGIN").await.unwrap();
        db.bulk_insert("devices", &["id".into(), "geom".into()], vec![Value::Integer(2), Value::Text("POINT(5 5)".into())], 1).await.unwrap();
        db.execute("UPDATE zones SET geom = ST_MakeEnvelope(50,50,60,60)").await.unwrap();
        assert_eq!(db.changefeeds().latest_sequence(), committed);
        std::fs::write(std::path::Path::new(&path).join("prefix"), format!("{prefix},{cursor}")).unwrap();
        std::process::exit(0);
    });
}

#[test]
fn data_and_events_recover_together_from_one_wal_commit() {
    use nexusql::storage::disk::WAL_FILE_NAME;
    use nexusql::storage::wal::{SyncPolicy, WriteAheadLog, FRAME_HEADER_LEN};
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;

    let source = tempfile::tempdir().unwrap();
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "changefeed_wal_child", "--nocapture"])
        .env("NEXUSQL_CHANGEFEED_WAL_CHILD", source.path())
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "{}",
        String::from_utf8_lossy(&child.stderr)
    );
    let prefix = fs::read_to_string(source.path().join("prefix")).unwrap();
    let (prefix, cursor) = prefix.split_once(',').unwrap();
    let prefix: usize = prefix.parse().unwrap();
    let cursor: u64 = cursor.parse().unwrap();
    let bytes = fs::read(source.path().join("default").join(WAL_FILE_NAME)).unwrap();

    // Inspect the actual redo payload, not just the final recovered tables:
    // device + camera + feed-log keys must share one and only one new commit.
    let copy = tempfile::tempdir().unwrap();
    let copy_path = copy.path().join(WAL_FILE_NAME);
    fs::write(&copy_path, &bytes[..prefix]).unwrap();
    let (wal, before) = WriteAheadLog::open(&copy_path, SyncPolicy::Full).unwrap();
    drop(wal);
    fs::write(&copy_path, &bytes).unwrap();
    let (wal, after) = WriteAheadLog::open(&copy_path, SyncPolicy::Full).unwrap();
    drop(wal);
    assert_eq!(after.len(), before.len() + 1);
    let changes: BTreeMap<Vec<u8>, Option<Vec<u8>>> =
        bincode::deserialize(after.last().unwrap()).unwrap();
    for name in ["devices", "cameras", nexusql::changefeed::LOG_TABLE] {
        assert!(changes.contains_key(format!("table:{name}:meta").as_bytes()));
    }

    // Check both sides of every frame/payload boundary and every byte of the
    // final commit marker. The WAL module separately exhausts all payload cuts.
    let mut cuts = BTreeSet::from([prefix, bytes.len()]);
    let mut offset = prefix;
    while offset < bytes.len() {
        let payload =
            u32::from_le_bytes(bytes[offset + 16..offset + 20].try_into().unwrap()) as usize;
        let end = offset + FRAME_HEADER_LEN + ((payload + 7) & !7);
        for cut in [
            offset,
            offset + 1,
            offset + FRAME_HEADER_LEN - 1,
            offset + FRAME_HEADER_LEN,
            end - 1,
            end,
        ] {
            cuts.insert(cut);
        }
        if payload == 0 {
            cuts.extend(offset..end);
        }
        offset = end;
    }
    let runtime = tokio::runtime::Runtime::new().unwrap();
    for cut in cuts {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("default")).unwrap();
        fs::write(
            root.path().join("default").join(WAL_FILE_NAME),
            &bytes[..cut],
        )
        .unwrap();
        runtime.block_on(async {
            let db = NexusSQL::new(root.path().to_str().unwrap()).await.unwrap();
            let complete = cut == bytes.len();
            let rows = db
                .execute("SELECT id, ST_AsText(geom) AS geom FROM devices")
                .await
                .unwrap()
                .rows;
            assert_eq!(rows.len(), 1, "uncommitted row at cut {cut}");
            assert_eq!(
                rows[0].get("geom"),
                Some(&Value::Text(
                    if complete {
                        "POINT(15 5)"
                    } else {
                        "POINT(5 5)"
                    }
                    .into()
                ))
            );
            assert_eq!(
                db.execute("SELECT * FROM cameras")
                    .await
                    .unwrap()
                    .rows
                    .len(),
                usize::from(complete)
            );
            let edits = db.changes("f", Some(cursor)).unwrap();
            let occupancy = db.changes("occupancy", Some(cursor)).unwrap();
            if complete {
                assert_eq!(
                    operations(&edits),
                    vec![Operation::Update, Operation::Insert]
                );
                assert_eq!(
                    operations(&occupancy),
                    vec![Operation::Exit, Operation::Enter]
                );
                assert_eq!(edits[0].commit_sequence, occupancy[1].commit_sequence);
                assert_eq!(zone(&occupancy[0]), &Value::Text("A".into()));
                assert_eq!(zone(&occupancy[1]), &Value::Text("B".into()));
            } else {
                assert!(edits.is_empty(), "orphan row event at cut {cut}");
                assert!(occupancy.is_empty(), "orphan geofence event at cut {cut}");
                assert_eq!(db.changefeeds().latest_sequence(), cursor);
            }
            db.shutdown().await.unwrap();
        });
    }
}
