//! Real child-process kills plus exhaustive torn-tail recovery tests.
use nexusql::index::GiSTPredicate;
use nexusql::storage::disk::{DiskStorage, WAL_FILE_NAME};
use nexusql::storage::wal::{SyncPolicy, WriteAheadLog, FRAME_HEADER_LEN, WAL_HEADER_LEN};
use nexusql::storage::{IndexType, Table};
use nexusql::{NexusSQL, Row, Value};
use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;

async fn seed(db: &NexusSQL) {
    db.execute("CREATE TABLE devices (id INT, label TEXT, geom GEOMETRY)")
        .await
        .unwrap();
    db.execute("CREATE INDEX devices_geom ON devices USING gist (geom)")
        .await
        .unwrap();
    // Literal INSERT takes the direct bulk path, including geometry coercion.
    db.execute("INSERT INTO devices VALUES (1, 'one', 'POINT(1 1)'), (2, 'two', 'POINT(2 2)'), (3, 'three', 'POINT(3 3)')")
        .await.unwrap();
    db.execute("CREATE TABLE mirror (id INT)").await.unwrap();
    db.execute("INSERT INTO mirror VALUES (1)").await.unwrap();
}

async fn edit(db: &NexusSQL) {
    db.execute("BEGIN").await.unwrap();
    db.execute("INSERT INTO devices VALUES (4, 'four', 'POINT(4 4)')")
        .await
        .unwrap();
    // Parameterized INSERT takes the regular Executor path.
    db.execute_with_params(
        "INSERT INTO mirror VALUES ($1)",
        Some(vec![Value::Integer(4)]),
    )
    .await
    .unwrap();
    db.execute("UPDATE devices SET label = 'changed', geom = ST_Point(9,9) WHERE id = 1")
        .await
        .unwrap();
    db.execute("DELETE FROM devices WHERE id = 2")
        .await
        .unwrap();
}

async fn uncommitted_edits(db: &NexusSQL) {
    db.execute("BEGIN").await.unwrap();
    db.execute("INSERT INTO devices VALUES (99, 'uncommitted', 'POINT(99 99)')")
        .await
        .unwrap();
    db.execute("UPDATE devices SET geom = ST_Point(100,100) WHERE id = 1")
        .await
        .unwrap();
    db.execute("DELETE FROM devices WHERE id = 3")
        .await
        .unwrap();
    db.execute("DROP TABLE mirror").await.unwrap();
    db.execute("CREATE TABLE uncommitted (id INT)")
        .await
        .unwrap();
}

// Invoked by the harness with an environment-selected scenario. The parent
// kills this process; no Drop/shutdown or unwinding can make writes durable.
#[test]
fn durability_child() {
    let Ok(mode) = std::env::var("NEXUSQL_DURABILITY_CHILD") else {
        return;
    };
    let root = std::env::var("NEXUSQL_DURABILITY_PATH").unwrap();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let db = NexusSQL::new(&root).await.unwrap();
        seed(&db).await;
        if mode != "tail" {
            db.flush().await.unwrap();
        }
        if mode == "normal" {
            db.execute("PRAGMA synchronous = NORMAL").await.unwrap();
        }
        let wal = Path::new(&root).join("default").join(WAL_FILE_NAME);
        let prefix = fs::metadata(&wal).unwrap().len();
        edit(&db).await;
        if mode == "rollback" {
            db.execute("ROLLBACK").await.unwrap();
        } else if mode != "uncommitted" {
            db.execute("COMMIT").await.unwrap();
            uncommitted_edits(&db).await;
        }
        if mode == "flush_active" || mode == "uncommitted" {
            db.flush().await.unwrap();
        }
        fs::write(Path::new(&root).join("ready"), prefix.to_string()).unwrap();
        loop {
            std::thread::park();
        }
    });
}

struct KillOnDrop(Child);
impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn kill_at_ready(root: &Path, mode: &str) -> usize {
    let log_path = root.join("child-output.txt");
    let output = fs::File::create(&log_path).unwrap();
    let mut child = KillOnDrop(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "durability_child", "--nocapture"])
            .env("NEXUSQL_DURABILITY_CHILD", mode)
            .env("NEXUSQL_DURABILITY_PATH", root)
            .stdout(Stdio::from(output.try_clone().unwrap()))
            .stderr(Stdio::from(output))
            .spawn()
            .unwrap(),
    );
    let started = Instant::now();
    let ready = root.join("ready");
    while !ready.exists() {
        if let Some(status) = child.0.try_wait().unwrap() {
            panic!(
                "child exited {status}: {}",
                fs::read_to_string(&log_path).unwrap()
            );
        }
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "child timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let prefix = fs::read_to_string(ready).unwrap().parse().unwrap();
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    prefix
}

fn ids(table: &Table) -> Vec<i64> {
    table
        .rows
        .iter()
        .map(|row| match row.get("id").unwrap() {
            Value::Integer(id) => *id,
            _ => panic!("non-integer id"),
        })
        .collect()
}

fn assert_recovered(disk: &DiskStorage, changed: bool) {
    let devices = disk.load_table("devices").unwrap().unwrap();
    assert_eq!(
        ids(&devices),
        if changed {
            vec![1, 3, 4]
        } else {
            vec![1, 2, 3]
        }
    );
    assert_eq!(
        devices.rows[0].get("label"),
        Some(&Value::Text(if changed { "changed" } else { "one" }.into()))
    );
    let IndexType::GiST(index) = &devices.typed_indexes["devices_geom"] else {
        panic!("GiST definition did not recover");
    };
    // Compare every point probe with the recovered table, including deleted and
    // uncommitted locations. Index IDs must remain correct after row compaction.
    for point in [1, 2, 3, 4, 9, 99, 100] {
        let expected: Vec<usize> = devices
            .rows
            .iter()
            .enumerate()
            .filter_map(|(i, row)| {
                let Value::Geometry(g) = row.get("geom").unwrap() else {
                    panic!()
                };
                (g.as_text() == format!("POINT({point} {point})")).then_some(i)
            })
            .collect();
        let mut actual = index.search(&GiSTPredicate::Point {
            x: point as f64,
            y: point as f64,
        });
        actual.sort_unstable();
        assert_eq!(actual, expected, "index mismatch at POINT({point} {point})");
    }
    assert_eq!(
        ids(&disk.load_table("mirror").unwrap().unwrap()),
        if changed { vec![1, 4] } else { vec![1] }
    );
    assert!(disk.load_table("uncommitted").unwrap().is_none());
}

/// Inspect sled directly BEFORE WAL recovery, to prove no dirty SQL table was
/// written through. This opens after the child died, releasing its sled lock.
fn main_device_ids(path: &Path) -> Vec<i64> {
    let db = sled::open(path).unwrap();
    let mut result = Vec::new();
    for chunk in db.scan_prefix(b"table:devices:chunk:") {
        let bytes = chunk.unwrap().1;
        let raw = zstd::decode_all(&bytes[..]).unwrap();
        let rows: Vec<Vec<Value>> = bincode::deserialize(&raw).unwrap();
        result.extend(rows.into_iter().map(|row| match row[0] {
            Value::Integer(id) => id,
            _ => panic!(),
        }));
    }
    result
}

#[test]
fn process_kill_preserves_commits_and_discards_uncommitted_dml() {
    for mode in [
        "committed",
        "normal",
        "uncommitted",
        "rollback",
        "flush_active",
    ] {
        let root = TempDir::new().unwrap();
        kill_at_ready(root.path(), mode);
        let path = root.path().join("default");
        assert_eq!(
            main_device_ids(&path),
            if mode == "flush_active" {
                vec![1, 3, 4]
            } else {
                vec![1, 2, 3]
            },
            "main file leaked in {mode}"
        );
        let disk = DiskStorage::new(path.to_str().unwrap()).unwrap();
        assert_recovered(&disk, !matches!(mode, "uncommitted" | "rollback"));
    }
}

#[test]
fn every_byte_of_last_transaction_is_a_safe_crash_boundary() {
    let source = TempDir::new().unwrap();
    let prefix = kill_at_ready(source.path(), "tail");
    let bytes = fs::read(source.path().join("default").join(WAL_FILE_NAME)).unwrap();
    let started = Instant::now();
    for cut in prefix..=bytes.len() {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join(WAL_FILE_NAME), &bytes[..cut]).unwrap();
        let disk = DiskStorage::new(root.path().to_str().unwrap()).unwrap();
        assert_recovered(&disk, cut == bytes.len());
        // A new commit after tail repair must not incorporate abandoned frames.
        if cut == prefix || cut == bytes.len() - 1 {
            let mut table = disk.load_table("mirror").unwrap().unwrap();
            table.insert(Row::new(vec!["id".into()], vec![Value::Integer(8)]));
            disk.save_table(&table).unwrap();
            disk.close().unwrap();
            drop(disk);
            let reopened = DiskStorage::new(root.path().to_str().unwrap()).unwrap();
            assert_eq!(
                ids(&reopened.load_table("mirror").unwrap().unwrap()),
                vec![1, 8]
            );
        }
    }
    eprintln!(
        "torn-tail recovery: {} byte boundaries, WAL {} bytes, {:.3}s",
        bytes.len() - prefix + 1,
        bytes.len(),
        started.elapsed().as_secs_f64()
    );
}

#[test]
fn checksum_rejects_each_corrupted_tail_byte_and_stale_salt() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("test.wal");
    let (mut wal, _) = WriteAheadLog::open(&path, SyncPolicy::Full).unwrap();
    wal.append(b"previous committed payload").unwrap();
    let prefix = wal.len() as usize;
    wal.append(b"last transaction data").unwrap();
    drop(wal);
    let original = fs::read(&path).unwrap();
    for at in prefix..original.len() {
        let mut damaged = original.clone();
        damaged[at] ^= 1;
        fs::write(&path, damaged).unwrap();
        let (wal, recovered) = WriteAheadLog::open(&path, SyncPolicy::Full).unwrap();
        assert_eq!(
            recovered,
            vec![b"previous committed payload".to_vec()],
            "byte {at}"
        );
        assert_eq!(wal.len() as usize, prefix);
    }
    // A valid, checksummed frame from a previous generation cannot be reused.
    let second = root.path().join("new.wal");
    let (wal, _) = WriteAheadLog::open(&second, SyncPolicy::Full).unwrap();
    drop(wal);
    let mut replacement = fs::read(&second).unwrap();
    replacement.extend_from_slice(&original[WAL_HEADER_LEN..]);
    fs::write(&second, replacement).unwrap();
    assert!(WriteAheadLog::open(&second, SyncPolicy::Full)
        .unwrap()
        .1
        .is_empty());
}

#[test]
fn large_transactions_require_all_frames_and_the_commit_marker() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("large.wal");
    let (mut wal, _) = WriteAheadLog::open(&path, SyncPolicy::Full).unwrap();
    wal.append(b"first").unwrap();
    let prefix = wal.len() as usize;
    let payload: Vec<u8> = (0..150_000).map(|n| (n % 251) as u8).collect();
    wal.append(&payload).unwrap();
    drop(wal);
    let original = fs::read(&path).unwrap();
    for cut in [
        prefix,
        prefix + FRAME_HEADER_LEN,
        prefix + 65_575,
        prefix + 65_576,
        original.len() - FRAME_HEADER_LEN,
        original.len() - 1,
        original.len(),
    ] {
        fs::write(&path, &original[..cut]).unwrap();
        let (_, recovered) = WriteAheadLog::open(&path, SyncPolicy::Full).unwrap();
        assert_eq!(recovered.len(), if cut == original.len() { 2 } else { 1 });
        assert_eq!(recovered[0], b"first");
        if cut == original.len() {
            assert_eq!(recovered[1], payload);
        }
    }
}

#[test]
fn torn_checkpoint_header_preserves_the_synced_main_database() {
    let root = TempDir::new().unwrap();
    let path = root.path().join(WAL_FILE_NAME);
    let disk = DiskStorage::new(root.path().to_str().unwrap()).unwrap();
    let mut table = Table::new("items".into(), vec!["id".into()]);
    table.insert(Row::new(vec!["id".into()], vec![Value::Integer(7)]));
    disk.save_table(&table).unwrap();
    disk.close().unwrap();
    drop(disk);
    let header = fs::read(&path).unwrap();
    for cut in 0..=WAL_HEADER_LEN {
        fs::write(&path, &header[..cut]).unwrap();
        let disk = DiskStorage::new(root.path().to_str().unwrap()).unwrap();
        assert_eq!(ids(&disk.load_table("items").unwrap().unwrap()), vec![7]);
    }
    let mut corrupt = header;
    corrupt[0] ^= 1;
    fs::write(&path, corrupt).unwrap();
    assert!(DiskStorage::new(root.path().to_str().unwrap()).is_err());
}

#[tokio::test]
async fn rollback_close_bulk_and_ddl_obey_transaction_boundaries() {
    let root = TempDir::new().unwrap();
    let db = NexusSQL::new(root.path().to_str().unwrap()).await.unwrap();
    seed(&db).await;
    db.set_bulk_mode(true).await;
    edit(&db).await;
    db.execute("COMMIT").await.unwrap();
    let wal = root.path().join("default").join(WAL_FILE_NAME);
    assert!(fs::metadata(&wal).unwrap().len() > WAL_HEADER_LEN as u64);
    uncommitted_edits(&db).await;
    db.flush().await.unwrap();
    db.execute("ROLLBACK").await.unwrap();
    db.execute("BEGIN").await.unwrap();
    db.execute("DROP TABLE mirror").await.unwrap();
    db.execute("COMMIT").await.unwrap();
    db.execute("BEGIN").await.unwrap();
    db.bulk_insert(
        "devices",
        &["id".into(), "label".into(), "geom".into()],
        vec![
            Value::Integer(80),
            Value::Text("pending".into()),
            Value::Text("POINT(80 80)".into()),
        ],
        1,
    )
    .await
    .unwrap();
    // shutdown rolls back that last transaction, even with bulk mode enabled.
    db.shutdown().await.unwrap();
    assert_eq!(fs::metadata(&wal).unwrap().len(), WAL_HEADER_LEN as u64);
    let disk = DiskStorage::new(root.path().join("default").to_str().unwrap()).unwrap();
    assert_eq!(
        ids(&disk.load_table("devices").unwrap().unwrap()),
        vec![1, 3, 4]
    );
    assert!(disk.load_table("mirror").unwrap().is_none());
    assert!(disk.load_table("uncommitted").unwrap().is_none());
}

#[tokio::test]
async fn sync_policy_and_automatic_checkpoint_are_real_configuration() {
    let root = TempDir::new().unwrap();
    let db = NexusSQL::new(root.path().to_str().unwrap()).await.unwrap();
    assert_eq!(
        db.execute("PRAGMA synchronous").await.unwrap().rows[0].values,
        vec![Value::Integer(2)]
    );
    for (mode, number) in [("normal", 1), ("off", 0), ("full", 2)] {
        db.execute(&format!("PRAGMA synchronous = {mode}"))
            .await
            .unwrap();
        assert_eq!(
            db.execute("PRAGMA synchronous").await.unwrap().rows[0].values,
            vec![Value::Integer(number)]
        );
    }
    for setting in ["'NORMAL'", "0", "2"] {
        db.execute(&format!("PRAGMA synchronous = {setting}"))
            .await
            .unwrap();
    }
    assert!(db
        .execute("PRAGMA synchronous = FULL trailing_junk")
        .await
        .is_err());
    assert!(db.execute("PRAGMA synchronous = invalid").await.is_err());
    assert!(db.execute("PRAGMA journal_mode = DELETE").await.is_err());
    db.execute("PRAGMA nexusql_wal_checkpoint_bytes = 1")
        .await
        .unwrap();
    seed(&db).await;
    let wal = root.path().join("default").join(WAL_FILE_NAME);
    assert_eq!(fs::metadata(&wal).unwrap().len(), WAL_HEADER_LEN as u64);
    db.execute("PRAGMA nexusql_wal_checkpoint_bytes = 0")
        .await
        .unwrap();
    edit(&db).await;
    assert!(db.execute("PRAGMA synchronous = OFF").await.is_err());
    db.execute("COMMIT").await.unwrap();
    assert!(fs::metadata(&wal).unwrap().len() > WAL_HEADER_LEN as u64);
    db.execute("PRAGMA wal_checkpoint").await.unwrap();
    assert_eq!(fs::metadata(&wal).unwrap().len(), WAL_HEADER_LEN as u64);
    db.shutdown().await.unwrap();
    let disk = DiskStorage::new(root.path().join("default").to_str().unwrap()).unwrap();
    assert_recovered(&disk, true);
}

#[tokio::test]
async fn failed_geometry_statement_cannot_leak_into_a_later_commit() {
    let root = TempDir::new().unwrap();
    let db = NexusSQL::new(root.path().to_str().unwrap()).await.unwrap();
    seed(&db).await;
    db.execute("BEGIN").await.unwrap();
    assert!(db
        .execute(
            "INSERT INTO devices VALUES (5, 'valid', 'POINT(5 5)'), (6, 'invalid', 'bad geometry')"
        )
        .await
        .is_err());
    assert!(db
        .execute_with_params(
            "INSERT INTO devices VALUES ($1, $2, $3), ($4, $5, $6)",
            Some(vec![
                Value::Integer(5),
                Value::Text("valid".into()),
                Value::Text("POINT(5 5)".into()),
                Value::Integer(6),
                Value::Text("invalid".into()),
                Value::Text("bad geometry".into())
            ])
        )
        .await
        .is_err());
    db.execute("COMMIT").await.unwrap();
    db.shutdown().await.unwrap();
    let disk = DiskStorage::new(root.path().join("default").to_str().unwrap()).unwrap();
    assert_recovered(&disk, false);
}
