"""HNSW vector index from Python: create_vector_index() and vector_search()."""

import math
import random

import pytest

from nexusql import NexusDB


def unit(v):
    n = math.sqrt(sum(x * x for x in v))
    return [x / n for x in v]


@pytest.fixture
def db(tmp_path):
    rng = random.Random(7)
    db = NexusDB(str(tmp_path / "db"))
    db.execute("CREATE TABLE docs (id INT, project TEXT, embedding VECTOR(16))")
    db.create_vector_index("docs", "embedding")
    rows = [[i, f"p{i % 3}", unit([rng.gauss(0, 1) for _ in range(16)])] for i in range(300)]
    db.bulk_insert("docs", ["id", "project", "embedding"], rows)
    yield db, rows
    db.close()


def test_nearest_vector_is_itself(db):
    db, rows = db
    hits = db.vector_search("docs", rows[42][2], select=["id"], limit=3)
    assert hits[0]["id"] == 42
    assert hits[0]["distance"] == pytest.approx(0.0, abs=1e-5)
    assert [h["distance"] for h in hits] == sorted(h["distance"] for h in hits)


def test_where_filter_and_reopen(db, tmp_path):
    db, rows = db
    hits = db.vector_search("docs", rows[10][2], select=["id", "project"],
                            where="project = ?", params=["p2"], limit=5)
    assert len(hits) == 5
    assert all(h["project"] == "p2" for h in hits)
    db.close()
    again = NexusDB(str(tmp_path / "db"))
    try:
        assert again.vector_search("docs", rows[99][2], select=["id"], limit=1)[0]["id"] == 99
    finally:
        again.close()


def test_rejects_bad_arguments(db):
    db, _ = db
    with pytest.raises(ValueError):
        db.create_vector_index("docs", "embedding", metric="manhattan")
    with pytest.raises(ValueError):
        db.create_vector_index("docs", "embedding", m=0)
    with pytest.raises(ValueError):
        db.vector_search("docs; DROP TABLE docs", [0.0] * 16)
