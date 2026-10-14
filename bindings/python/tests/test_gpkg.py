"""Import/export/round-trip tests for nexusql.gpkg and nexusql.geojson.

Uses tests/fixtures/kfm_subset.gpkg, a small deterministic copy of a few
rows from KFM's real dist/kfm.gpkg (see fixtures/make_fixture.py), so
these tests do not depend on that external path.
"""

import json
import os

import pytest

from nexusql import NexusDB, geojson, gpkg

FIXTURE = os.path.join(os.path.dirname(__file__), "fixtures", "kfm_subset.gpkg")


@pytest.fixture
def db():
    with NexusDB(":memory:") as database:
        yield database


def test_import_gpkg_creates_tables_with_indexes(db):
    counts = gpkg.import_gpkg(db, FIXTURE)
    assert counts == {"zones": 5, "devices": 8, "cameras": 4}

    zones = db.execute("SELECT fid, code FROM zones ORDER BY fid").rows
    assert [r["fid"] for r in zones] == [1, 2, 3, 4, 5]
    assert all(r["code"] for r in zones)

    # GiST index exists and is usable (ST_Contains plans through it).
    hit = db.execute(
        "SELECT z.code FROM zones z JOIN devices d ON ST_Contains(z.geom, d.geom)"
    ).rows
    assert isinstance(hit, list)


def test_import_gpkg_layer_subset(db):
    counts = gpkg.import_gpkg(db, FIXTURE, layers=["zones"])
    assert counts == {"zones": 5}
    assert db.execute("SELECT * FROM zones").rows


def test_import_gpkg_unknown_layer_raises(db):
    with pytest.raises(ValueError, match="not found"):
        gpkg.import_gpkg(db, FIXTURE, layers=["not_a_layer"])


def test_export_gpkg_round_trip(db, tmp_path):
    gpkg.import_gpkg(db, FIXTURE, layers=["zones"])
    out_path = str(tmp_path / "zones_export.gpkg")

    written = gpkg.export_gpkg(db, "zones", out_path, "zones")
    assert written == 5

    db2 = NexusDB(":memory:")
    try:
        counts = gpkg.import_gpkg(db2, out_path)
        assert counts == {"zones": 5}

        before = db.execute(
            "SELECT fid, code, ST_AsText(geom) AS g FROM zones ORDER BY fid"
        ).rows
        after = db2.execute(
            "SELECT fid, code, ST_AsText(geom) AS g FROM zones ORDER BY fid"
        ).rows
        assert before == after
    finally:
        db2.close()


def test_export_gpkg_accepts_select_query(db, tmp_path):
    gpkg.import_gpkg(db, FIXTURE, layers=["zones"])
    out_path = str(tmp_path / "zones_filtered.gpkg")

    written = gpkg.export_gpkg(
        db, "SELECT fid, code, geom FROM zones WHERE fid <= 2", out_path, "zones"
    )
    assert written == 2


def test_import_geojson_round_trips_through_export(db, tmp_path):
    gpkg.import_gpkg(db, FIXTURE, layers=["devices"])
    geojson_path = str(tmp_path / "devices.geojson")

    written = geojson.export_geojson(db, "devices", geojson_path)
    assert written == 8

    with open(geojson_path, "r", encoding="utf-8") as f:
        data = json.load(f)
    assert data["type"] == "FeatureCollection"
    assert len(data["features"]) == 8
    assert data["features"][0]["geometry"]["type"] == "Point"

    db2 = NexusDB(":memory:")
    try:
        imported = geojson.import_geojson(db2, geojson_path, "devices")
        assert imported == 8

        before = db.execute(
            "SELECT fid, code, ST_AsText(geom) AS g FROM devices ORDER BY fid"
        ).rows
        after = db2.execute(
            "SELECT fid, code, ST_AsText(geom) AS g FROM devices ORDER BY fid"
        ).rows
        assert before == after
    finally:
        db2.close()


def test_import_geojson_infers_property_types(db, tmp_path):
    path = str(tmp_path / "mini.geojson")
    with open(path, "w", encoding="utf-8") as f:
        json.dump(
            {
                "type": "FeatureCollection",
                "features": [
                    {
                        "type": "Feature",
                        "geometry": {"type": "Point", "coordinates": [1.0, 2.0]},
                        "properties": {"name": "a", "count": 3, "ratio": 0.5},
                    },
                    {
                        "type": "Feature",
                        "geometry": {"type": "Point", "coordinates": [3.0, 4.0]},
                        "properties": {"name": "b", "count": 4, "ratio": 1.5},
                    },
                ],
            },
            f,
        )
    imported = geojson.import_geojson(db, path, "mini")
    assert imported == 2
    rows = db.execute("SELECT name, count, ratio, ST_AsText(geom) AS g FROM mini ORDER BY name").rows
    assert rows == [
        {"name": "a", "count": 3, "ratio": 0.5, "g": "POINT(1 2)"},
        {"name": "b", "count": 4, "ratio": 1.5, "g": "POINT(3 4)"},
    ]


def test_rejects_quoted_style_identifiers(db, tmp_path):
    # NexusQL's CREATE INDEX ... USING gist does not resolve quoted
    # identifiers (see gpkg._quote_ident docstring) -- a column name
    # that is not a plain SQL identifier must be rejected up front
    # rather than silently producing a broken index.
    path = str(tmp_path / "bad.geojson")
    with open(path, "w", encoding="utf-8") as f:
        json.dump(
            {
                "type": "FeatureCollection",
                "features": [
                    {
                        "type": "Feature",
                        "geometry": {"type": "Point", "coordinates": [0.0, 0.0]},
                        "properties": {"bad name": 1},
                    }
                ],
            },
            f,
        )
    with pytest.raises(ValueError, match="not a plain SQL identifier"):
        geojson.import_geojson(db, path, "bad")
