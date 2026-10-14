from concurrent.futures import ThreadPoolExecutor

import pytest

from nexusql import NexusDB


@pytest.fixture
def db():
    with NexusDB(":memory:") as database:
        database.execute("CREATE TABLE devices (id INT, geom GEOMETRY)")
        database.execute("CREATE CHANGEFEED live ON devices")
        yield database


def test_changes_exact_payload_and_exclusive_cursor(db):
    db.execute("INSERT INTO devices VALUES ($1, ST_Point($2,$3))", [1, 5, 5])
    event, = db.changes("live")
    assert event["operation"] == "insert"
    assert event["old"] is None
    assert event["new"] == {"id": 1, "geom": "POINT(5 5)"}
    assert event["zone"] is None
    assert event["row_id"] == 1
    assert event["timestamp_ms"] > 0
    assert db.changes("live", event["sequence"]) == []
    db.execute("UPDATE devices SET geom = ST_Point(6,6)")
    updated, = db.changes("live", event["sequence"])
    assert updated["old"] == event["new"]
    assert updated["new"] == {"id": 1, "geom": "POINT(6 6)"}
    assert updated["row_id"] == event["row_id"]


def test_transaction_and_bulk_insert_rollback_are_silent(db):
    db.execute("BEGIN")
    db.bulk_insert("devices", ["id", "geom"], [(1, "POINT(1 1)")])
    assert db.changes("live") == []
    db.execute("ROLLBACK")
    assert db.changes("live") == []
    db.execute("BEGIN")
    db.bulk_insert("devices", ["id", "geom"], [(2, "POINT(2 2)")])
    db.execute("UPDATE devices SET geom = ST_Point(3,3)")
    db.execute("COMMIT")
    event, = db.changes("live")
    assert event["operation"] == "insert"
    assert event["new"] == {"id": 2, "geom": "POINT(3 3)"}


def test_iterator_catchup_timeout_and_gil_release(db):
    db.execute("INSERT INTO devices VALUES (1, 'POINT(1 1)')")
    watcher = db.watch_changes("live", since=0)
    assert next(watcher)["sequence"] == 1
    assert watcher.recv(timeout=0) is None
    with ThreadPoolExecutor(max_workers=1) as executor:
        waiting = executor.submit(watcher.recv, 5)
        db.execute("UPDATE devices SET geom = ST_Point(2,2)")
        assert waiting.result(timeout=6)["operation"] == "update"
    with pytest.raises(ValueError):
        watcher.recv(timeout=-1)


def test_retention_and_drop_raise_explicit_errors(db):
    db.set_changefeed_retention(1)
    watcher = db.watch_changes("live")
    db.execute("INSERT INTO devices VALUES (1, NULL), (2, NULL)")
    assert len(db.changes("live")) == 1
    with pytest.raises(RuntimeError, match="expired"):
        db.changes("live", since=0)
    with pytest.raises(RuntimeError, match="expired"):
        watcher.recv(timeout=0)
    db.execute("DROP CHANGEFEED live")
    with pytest.raises(RuntimeError, match="does not exist"):
        db.changes("live")


def test_geofence_boundary_adjacent_move_and_zone_edits(db):
    db.execute("CREATE TABLE zones (code TEXT, geom GEOMETRY)")
    db.execute("INSERT INTO zones VALUES ('A', ST_MakeEnvelope(0,0,10,10)), ('B', ST_MakeEnvelope(10,0,20,10))")
    db.execute("CREATE GEOFENCE occupancy ON devices(geom) ZONES zones(geom)")
    db.execute("INSERT INTO devices VALUES (1, ST_Point(10,5))")
    events = db.changes("occupancy")
    assert [(e["operation"], e["zone"]["values"]["code"]) for e in events] == [("enter", "A"), ("enter", "B")]
    db.execute("UPDATE devices SET geom = ST_Point(5,5)")
    cursor = db.changes("occupancy")[-1]["sequence"]
    db.execute("UPDATE devices SET geom = ST_Point(15,5)")
    events = db.changes("occupancy", cursor)
    assert [(e["operation"], e["zone"]["values"]["code"]) for e in events] == [("exit", "A"), ("enter", "B")]
    assert events[0]["commit_sequence"] == events[1]["commit_sequence"]
    db.execute("DELETE FROM zones WHERE code = 'B'")
    event, = db.changes("occupancy", events[-1]["sequence"])
    assert event["operation"] == "exit"


def test_reopen_retains_feed_history(tmp_path):
    path = str(tmp_path / "factory")
    with NexusDB(path) as db:
        db.execute("CREATE TABLE devices (id INT)")
        db.execute("CREATE CHANGEFEED live ON devices")
        db.execute("INSERT INTO devices VALUES (1)")
        original = db.changes("live")
    with NexusDB(path) as db:
        assert db.changes("live") == original
        db.execute("INSERT INTO devices VALUES (2)")
        event, = db.changes("live", original[-1]["sequence"])
        assert event["new"] == {"id": 2}


def test_closed_database_rejects_feed_calls():
    db = NexusDB(":memory:")
    db.close()
    with pytest.raises(RuntimeError, match="closed"):
        db.changes("live")
    with pytest.raises(RuntimeError, match="closed"):
        db.watch_changes("live")
