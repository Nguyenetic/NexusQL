"""Full-text search from Python: the SQL surface and the search() helper."""

import pytest

from nexusql import NexusDB

MESSAGES = [
    (1, "nexus", "claude", "2026-10-01", "Rust rust RUST database engine"),
    (2, "nexus", "codex", "2026-10-02", "Learning Rust today with a quick-start guide"),
    (3, "nexus", "claude", "2026-10-03", "Python only, no compiled languages here"),
    (4, "kmes", "claude", "2026-10-04", "Rust in another project entirely"),
    (5, "kmes", "kimi", "2026-10-05", "The quick brown fox jumps over the lazy dog"),
    (6, "kmes", "codex", "2026-10-06", "東京タワーは東京都港区にあります"),
    (7, "amem", "claude", "2026-10-07", "Shared agent memory with bm25 ranking"),
]


def load(db):
    db.execute(
        "CREATE TABLE messages (id INT, project TEXT, provider TEXT, created TEXT, content TEXT)"
    )
    db.create_fulltext_index("messages", "content")
    for row in MESSAGES:
        # ? placeholders bind in order, as with sqlite3.
        db.execute("INSERT INTO messages VALUES (?, ?, ?, ?, ?)", list(row))


def ids(rows):
    return [r["id"] for r in rows]


def test_sql_match_bm25_and_snippet():
    with NexusDB(":memory:") as db:
        load(db)
        rows = db.execute(
            "SELECT id, bm25(content, $1) AS score, snippet(content, $1, '<b>', '</b>', '...', 4) AS s "
            "FROM messages WHERE content MATCH $1 AND project = $2 ORDER BY score LIMIT 5",
            ["rust", "nexus"],
        ).rows
        assert ids(rows) == [1, 2]
        assert rows[0]["score"] < rows[1]["score"] < 0
        assert rows[1]["s"] == "Learning <b>Rust</b> today with..."
        assert ids(db.execute("SELECT id FROM messages WHERE content MATCH '\"learning rust\"'").rows) == [2]
        assert ids(db.execute("SELECT id FROM messages WHERE content MATCH 'rust NOT project'").rows) == [1, 2]
        assert ids(db.execute("SELECT id FROM messages WHERE content MATCH 'lang*'").rows) == [3]
        assert ids(db.execute("SELECT id FROM messages WHERE content MATCH '港区'").rows) == [6]
        with pytest.raises(RuntimeError, match="invalid full-text query"):
            db.execute("SELECT id FROM messages WHERE content MATCH '(oops'")


def test_search_helper_filters_ranks_and_highlights():
    with NexusDB(":memory:") as db:
        load(db)
        rows = db.search(
            "messages",
            "rust OR quick",
            select=["id", "provider"],
            where="provider IN (?, ?) AND created >= ?",
            params=["claude", "codex", "2026-10-02"],
            limit=2,
            snippet_tokens=5,
            highlight=("**", "**"),
        )
        assert ids(rows) == [2, 4]
        assert rows[0]["snippet"] == "...**Rust** today with a **quick**..."
        assert set(rows[0]) == {"id", "provider", "score", "snippet"}
        assert db.search("messages", "absent words") == []
        with pytest.raises(ValueError):
            db.search("messages; DROP TABLE messages", "rust")


def test_reimport_rollback_and_reopen(tmp_path):
    path = str(tmp_path / "amem")
    with NexusDB(path) as db:
        load(db)
        # Delete and re-import one source atomically.
        with db.transaction():
            deleted = db.execute("DELETE FROM messages WHERE project = ?", ["kmes"])
            assert deleted.rows_affected == 3
            db.bulk_insert(
                "messages",
                ["id", "project", "content"],
                [(8, "kmes", "reimported zebra message"), (9, "kmes", "another zebra")],
            )
        # A failed re-import rolls back, leaving the index untouched.
        with pytest.raises(RuntimeError):
            with db.transaction():
                db.execute("DELETE FROM messages WHERE project = 'kmes'")
                raise RuntimeError("import failed")
        assert ids(db.search("messages", "zebra", snippet_tokens=None)) == [9, 8]
    with NexusDB(path) as db:
        rows = db.search("messages", "zebra", select=["id"], snippet_tokens=None)
        assert sorted(ids(rows)) == [8, 9]
        assert ids(db.execute("SELECT id FROM messages WHERE content MATCH 'fox'").rows) == []
