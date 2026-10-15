//! End-to-end full-text search through SQL: index maintenance, ranking,
//! snippets, transactions, durability across reopen and process kills.
use nexusql::{NexusSQL, ResultSet, Value};
use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;

const SCHEMA: &str =
    "CREATE TABLE messages (id INT, project TEXT, provider TEXT, created TEXT, content TEXT)";

fn ids(result: &ResultSet) -> Vec<i64> {
    result
        .rows
        .iter()
        .map(|row| row.get("id").and_then(Value::as_i64).unwrap())
        .collect()
}

fn text(result: &ResultSet, row: usize, column: &str) -> String {
    result.rows[row]
        .get(column)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

async fn insert(
    db: &NexusSQL,
    table: &str,
    id: i64,
    project: &str,
    provider: &str,
    created: &str,
    content: &str,
) {
    db.execute_with_params(
        &format!("INSERT INTO {table} VALUES ($1, $2, $3, $4, $5)"),
        Some(vec![
            Value::Integer(id),
            Value::Text(project.into()),
            Value::Text(provider.into()),
            Value::Text(created.into()),
            Value::Text(content.into()),
        ]),
    )
    .await
    .unwrap();
}

const CORPUS: &[(i64, &str, &str, &str, &str)] = &[
    (
        1,
        "nexus",
        "claude",
        "2026-10-01",
        "Rust rust RUST database engine",
    ),
    (
        2,
        "nexus",
        "codex",
        "2026-10-02",
        "Learning Rust today with a quick-start guide",
    ),
    (
        3,
        "nexus",
        "claude",
        "2026-10-03",
        "Python only, no compiled languages here",
    ),
    (
        4,
        "kmes",
        "claude",
        "2026-10-04",
        "Rust in another project entirely",
    ),
    (
        5,
        "kmes",
        "kimi",
        "2026-10-05",
        "The quick brown fox jumps over the lazy dog",
    ),
    (
        6,
        "kmes",
        "codex",
        "2026-10-06",
        "東京タワーは東京都港区にあります。日本語のテスト",
    ),
    (
        7,
        "amem",
        "claude",
        "2026-10-07",
        "Shared agent memory: search transcripts with bm25 ranking",
    ),
    (
        8,
        "amem",
        "kimi",
        "2026-10-08",
        "Café naïve résumé — diacritics fold to ASCII",
    ),
];

async fn seed(db: &NexusSQL, table: &str, indexed: bool) {
    db.execute(&SCHEMA.replace("messages", table))
        .await
        .unwrap();
    if indexed {
        db.execute(&format!(
            "CREATE INDEX {table}_fts ON {table} USING fulltext (content)"
        ))
        .await
        .unwrap();
    }
    for (id, project, provider, created, content) in CORPUS {
        insert(db, table, *id, project, provider, created, content).await;
    }
}

#[tokio::test]
async fn ranked_filtered_search_with_snippets() {
    let db = NexusSQL::new(":memory:").await.unwrap();
    seed(&db, "messages", true).await;

    let result = db
        .execute(
            "SELECT id, bm25(content, 'rust') AS score, \
                    snippet(content, 'rust', '<b>', '</b>', '...', 4) AS excerpt \
             FROM messages WHERE content MATCH 'rust' AND project = 'nexus' \
             ORDER BY score LIMIT 10",
        )
        .await
        .unwrap();
    assert_eq!(ids(&result), [1, 2], "filtered and best-first");
    let best = result.rows[0].get("score").and_then(Value::as_f64).unwrap();
    let next = result.rows[1].get("score").and_then(Value::as_f64).unwrap();
    assert!(
        best < next && next < 0.0,
        "FTS5 sign: lower is better ({best}, {next})"
    );
    assert_eq!(
        text(&result, 1, "excerpt"),
        "Learning <b>Rust</b> today with..."
    );

    // fts_score is the positive score for ORDER BY ... DESC, and LIMIT applies after ranking.
    let result = db
        .execute(
            "SELECT id, fts_score(content, 'rust') AS score FROM messages \
             WHERE content MATCH 'rust' ORDER BY score DESC LIMIT 2",
        )
        .await
        .unwrap();
    assert_eq!(ids(&result), [1, 4]);

    // Date and provider filters combined with OR inside MATCH.
    let result = db
        .execute(
            "SELECT id FROM messages WHERE content MATCH 'quick OR bm25' \
             AND created >= '2026-10-03' AND provider = 'claude'",
        )
        .await
        .unwrap();
    assert_eq!(ids(&result), [7]);

    let result = db
        .execute("SELECT id, highlight(content, '\"quick brown\" OR dog') AS h FROM messages WHERE id = 5")
        .await
        .unwrap();
    assert_eq!(
        text(&result, 0, "h"),
        "The [quick brown] fox jumps over the lazy [dog]"
    );
}

#[tokio::test]
async fn query_syntax_through_sql() {
    let db = NexusSQL::new(":memory:").await.unwrap();
    seed(&db, "messages", true).await;
    let cases: &[(&str, &[i64])] = &[
        ("rust", &[1, 2, 4]),
        ("\"learning rust\"", &[2]),
        ("\"rust learning\"", &[]),
        ("lang*", &[3]),
        ("rust NOT project", &[1, 2]),
        ("rust AND (guide OR engine)", &[1, 2]),
        ("quick", &[2, 5]),
        ("quick-start", &[2]),
        ("港区", &[6]),
        ("東京タワー", &[6]),
        ("港", &[6]),
        ("cafe resume", &[8]),
        ("CAFÉ", &[8]),
        ("fetch data", &[]),
        ("NOT rust", &[3, 5, 6, 7, 8]),
        ("...", &[]),
    ];
    for (query, expected) in cases {
        let result = db
            .execute_with_params(
                "SELECT id FROM messages WHERE content MATCH $1",
                Some(vec![Value::Text((*query).into())]),
            )
            .await
            .unwrap();
        assert_eq!(&ids(&result), expected, "query {query}");
    }
    // Function and PostgreSQL operator spellings are equivalent.
    let a = db
        .execute("SELECT id FROM messages WHERE fts_match(content, 'rust')")
        .await
        .unwrap();
    let b = db
        .execute("SELECT id FROM messages WHERE content @@ 'rust'")
        .await
        .unwrap();
    assert_eq!(ids(&a), [1, 2, 4]);
    assert_eq!(ids(&b), [1, 2, 4]);
    // Aggregates over a full-text filter.
    let count = db
        .execute("SELECT COUNT(*) AS n FROM messages WHERE content MATCH 'rust'")
        .await
        .unwrap();
    assert_eq!(count.rows[0].get("n").and_then(Value::as_i64), Some(3));

    let error = db
        .execute("SELECT id FROM messages WHERE content MATCH '(unbalanced'")
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("invalid full-text query"), "{error}");
    let error = db
        .execute("CREATE INDEX two ON messages USING fulltext (content, project)")
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("exactly one column"), "{error}");
}

const QUERIES: &[&str] = &[
    "rust",
    "rust*",
    "\"rust database\"",
    "quick OR fox",
    "NOT rust",
    "rust NOT guide",
    "東京",
    "replaced",
    "gone",
    "café",
    "agent memory",
];

async fn assert_same_results(db: &NexusSQL) {
    for query in QUERIES {
        let sql = |table: &str| {
            format!(
                "SELECT id, bm25(content, $1) AS score FROM {table} WHERE content MATCH $1 ORDER BY score"
            )
        };
        let params = || Some(vec![Value::Text((*query).into())]);
        let indexed = db
            .execute_with_params(&sql("messages"), params())
            .await
            .unwrap();
        let scanned = db
            .execute_with_params(&sql("plain"), params())
            .await
            .unwrap();
        assert_eq!(ids(&indexed), ids(&scanned), "query {query}");
        for (a, b) in indexed.rows.iter().zip(&scanned.rows) {
            assert_eq!(a.get("score"), b.get("score"), "query {query}");
        }
    }
}

#[tokio::test]
async fn index_stays_exact_through_dml_and_rollback() {
    let db = NexusSQL::new(":memory:").await.unwrap();
    seed(&db, "messages", true).await;
    seed(&db, "plain", false).await;
    assert_same_results(&db).await;

    for table in ["messages", "plain"] {
        db.execute(&format!(
            "UPDATE {table} SET content = 'replaced rust text' WHERE id = 3"
        ))
        .await
        .unwrap();
        db.execute(&format!("DELETE FROM {table} WHERE project = 'kmes'"))
            .await
            .unwrap();
        insert(
            &db,
            table,
            9,
            "amem",
            "claude",
            "2026-10-09",
            "agent memory gone soon",
        )
        .await;
        db.execute(&format!("UPDATE {table} SET content = NULL WHERE id = 7"))
            .await
            .unwrap();
    }
    assert_same_results(&db).await;

    // Re-import a source inside a transaction, then roll it back.
    let before = db
        .execute("SELECT id FROM messages WHERE content MATCH 'rust OR memory'")
        .await
        .unwrap();
    db.execute("BEGIN").await.unwrap();
    db.execute("DELETE FROM messages WHERE project = 'amem'")
        .await
        .unwrap();
    insert(
        &db,
        "messages",
        10,
        "amem",
        "claude",
        "2026-10-10",
        "rust memory reimported",
    )
    .await;
    db.execute("UPDATE messages SET content = 'no match' WHERE id = 1")
        .await
        .unwrap();
    let during = db
        .execute("SELECT id FROM messages WHERE content MATCH 'rust OR memory'")
        .await
        .unwrap();
    assert_eq!(ids(&during), [2, 3, 10]);
    db.execute("ROLLBACK").await.unwrap();
    let after = db
        .execute("SELECT id FROM messages WHERE content MATCH 'rust OR memory'")
        .await
        .unwrap();
    assert_eq!(ids(&after), ids(&before));
    assert_same_results(&db).await;

    // A statement error restores the table and its index.
    db.execute("UPDATE messages SET content = 'should not stay' WHERE id = 2")
        .await
        .unwrap();
    assert!(db.execute("INSERT INTO missing VALUES (1)").await.is_err());
    let result = db
        .execute("SELECT id FROM messages WHERE content MATCH 'stay'")
        .await
        .unwrap();
    assert_eq!(ids(&result), [2]);
}

#[tokio::test]
async fn bulk_insert_and_index_after_load() {
    let db = NexusSQL::new(":memory:").await.unwrap();
    db.execute(SCHEMA).await.unwrap();
    let columns: Vec<String> = ["id", "content"].iter().map(|c| c.to_string()).collect();
    let mut flat = Vec::new();
    for i in 0..500 {
        flat.push(Value::Integer(i));
        flat.push(Value::Text(format!("message {i} topic{}", i % 10)));
    }
    db.bulk_insert("messages", &columns, flat, 500)
        .await
        .unwrap();
    // Index built over existing rows, then maintained by later bulk loads.
    db.execute("CREATE INDEX m_fts ON messages USING fts5 (content)")
        .await
        .unwrap();
    db.bulk_insert(
        "messages",
        &columns,
        vec![
            Value::Integer(500),
            Value::Text("late topic3 arrival".into()),
        ],
        1,
    )
    .await
    .unwrap();
    let result = db
        .execute("SELECT id FROM messages WHERE content MATCH 'topic3'")
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 51);
    assert_eq!(*ids(&result).last().unwrap(), 500);
}

#[tokio::test]
async fn index_survives_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("db");
    let path = path.to_str().unwrap();
    {
        let db = NexusSQL::new(path).await.unwrap();
        seed(&db, "messages", true).await;
        db.execute("BEGIN").await.unwrap();
        db.execute("DELETE FROM messages WHERE id = 1")
            .await
            .unwrap();
        insert(
            &db,
            "messages",
            11,
            "amem",
            "claude",
            "2026-10-11",
            "committed rust addition",
        )
        .await;
        db.execute("COMMIT").await.unwrap();
        db.shutdown().await.unwrap();
    }
    {
        let db = NexusSQL::new(path).await.unwrap();
        let result = db
            .execute("SELECT id FROM messages WHERE content MATCH 'rust' ORDER BY id")
            .await
            .unwrap();
        assert_eq!(ids(&result), [2, 4, 11]);
        // The index is maintained after reopen (rebuilt from its definition).
        db.execute("UPDATE messages SET content = 'rust again' WHERE id = 3")
            .await
            .unwrap();
        let result = db
            .execute("SELECT id FROM messages WHERE content MATCH 'again'")
            .await
            .unwrap();
        assert_eq!(ids(&result), [3]);
        // Drop without shutdown: the autocommit above is already durable.
    }
    let db = NexusSQL::new(path).await.unwrap();
    let result = db
        .execute("SELECT id FROM messages WHERE content MATCH 'again'")
        .await
        .unwrap();
    assert_eq!(ids(&result), [3]);
    let error = db
        .execute("CREATE INDEX messages_fts ON messages USING fulltext (content)")
        .await;
    assert!(error.is_ok(), "re-creating replaces the definition");
    db.shutdown().await.unwrap();
}

// Child process for the kill test. The parent kills it while a transaction
// containing searchable rows is still open.
#[test]
fn fulltext_kill_child() {
    let Ok(root) = std::env::var("NEXUSQL_FTS_CHILD") else {
        return;
    };
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let db = NexusSQL::new(&root).await.unwrap();
        seed(&db, "messages", true).await;
        db.execute("BEGIN").await.unwrap();
        insert(
            &db,
            "messages",
            12,
            "amem",
            "claude",
            "2026-10-12",
            "uncommitted zebra text",
        )
        .await;
        db.execute("UPDATE messages SET content = 'zebra overwrite' WHERE id = 2")
            .await
            .unwrap();
        db.execute("DELETE FROM messages WHERE id = 4")
            .await
            .unwrap();
        db.execute("COMMIT").await.unwrap();
        db.execute("BEGIN").await.unwrap();
        insert(
            &db,
            "messages",
            13,
            "amem",
            "claude",
            "2026-10-13",
            "never committed giraffe",
        )
        .await;
        db.execute("DELETE FROM messages WHERE id = 12")
            .await
            .unwrap();
        fs::write(Path::new(&root).join("ready"), "1").unwrap();
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

#[tokio::test]
async fn killed_process_recovers_committed_text_only() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("db");
    let log = dir.path().join("child.txt");
    let output = fs::File::create(&log).unwrap();
    let mut child = KillOnDrop(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "fulltext_kill_child", "--nocapture"])
            .env("NEXUSQL_FTS_CHILD", &root)
            .stdout(Stdio::from(output.try_clone().unwrap()))
            .stderr(Stdio::from(output))
            .spawn()
            .unwrap(),
    );
    let started = Instant::now();
    while !root.join("ready").exists() {
        if let Some(status) = child.0.try_wait().unwrap() {
            panic!(
                "child exited {status}: {}",
                fs::read_to_string(&log).unwrap()
            );
        }
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "child timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    child.0.kill().unwrap();
    child.0.wait().unwrap();

    let db = NexusSQL::new(root.to_str().unwrap()).await.unwrap();
    let zebra = db
        .execute("SELECT id FROM messages WHERE content MATCH 'zebra'")
        .await
        .unwrap();
    assert_eq!(ids(&zebra), [2, 12], "committed transaction survives");
    let giraffe = db
        .execute("SELECT id FROM messages WHERE content MATCH 'giraffe'")
        .await
        .unwrap();
    assert!(giraffe.rows.is_empty(), "uncommitted insert is gone");
    let rust = db
        .execute("SELECT id FROM messages WHERE content MATCH 'rust'")
        .await
        .unwrap();
    assert_eq!(ids(&rust), [1], "committed update and delete are reflected");
}

#[tokio::test]
async fn failed_insert_statement_rolls_back_rows_and_index() {
    let db = NexusSQL::new(":memory:").await.unwrap();
    db.execute("CREATE TABLE notes (id SERIAL, body TEXT, geom GEOMETRY)")
        .await
        .unwrap();
    db.execute("CREATE INDEX notes_fts ON notes USING fulltext (body)")
        .await
        .unwrap();
    db.execute("INSERT INTO notes (body) VALUES ('kept walrus')")
        .await
        .unwrap();
    for in_transaction in [false, true] {
        if in_transaction {
            db.execute("BEGIN").await.unwrap();
        }
        // The second row's WKT is invalid after the first row was appended.
        let error = db
            .execute_with_params(
                "INSERT INTO notes (body, geom) VALUES ($1, $2), ($3, $4)",
                Some(vec![
                    Value::Text("orphan walrus".into()),
                    Value::Text("POINT(1 1)".into()),
                    Value::Text("second".into()),
                    Value::Text("NOT WKT".into()),
                ]),
            )
            .await;
        assert!(error.is_err());
        let result = db
            .execute("SELECT id FROM notes WHERE body MATCH 'walrus'")
            .await
            .unwrap();
        assert_eq!(ids(&result), [1], "in transaction: {in_transaction}");
        if in_transaction {
            db.execute("COMMIT").await.unwrap();
        }
    }
    db.execute("INSERT INTO notes (body) VALUES ('next walrus')")
        .await
        .unwrap();
    let result = db
        .execute("SELECT id FROM notes WHERE body MATCH 'walrus'")
        .await
        .unwrap();
    assert_eq!(ids(&result), [1, 2], "auto-increment restored");
}
