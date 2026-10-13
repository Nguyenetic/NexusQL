import pytest

from nexusql import NexusDB


@pytest.fixture
def db():
    with NexusDB(":memory:") as database:
        yield database


def test_geometry_values_are_wkt_and_execute_accepts_parameters(db):
    db.execute("CREATE TABLE devices (name TEXT, geom GEOMETRY, active BOOLEAN)")
    name = "camera'; DROP TABLE devices; --"
    db.execute("INSERT INTO devices VALUES ($1, ST_GeomFromText($2), $3)", (name, "POINT(3 4)", True))
    assert db.execute("SELECT * FROM devices").rows == [
        {"name": name, "geom": "POINT(3 4)", "active": True}
    ]
    assert db.execute("SELECT ST_Distance(geom, ST_Point($1,$2)) AS distance FROM devices", [0, 0]).rows == [{"distance": 5.0}]


@pytest.mark.parametrize("function,point,expected", [
    ("ST_Contains", "POINT(5 5)", True),
    ("ST_Contains", "POINT(10 5)", False),
    ("ST_Intersects", "POINT(10 5)", True),
    ("ST_Intersects", "POINT(11 5)", False),
])
def test_boundary_semantics(db, function, point, expected):
    rows = db.execute(f"SELECT {function}(ST_MakeEnvelope(0,0,10,10), ST_GeomFromText($1)) AS result", [point]).rows
    assert rows == [{"result": expected}]


def test_spatial_join_and_index_maintenance(db):
    db.execute("CREATE TABLE zones (code TEXT, geom GEOMETRY)")
    db.execute("CREATE TABLE devices (name TEXT, geom GEOMETRY)")
    db.execute("INSERT INTO zones VALUES ('A', ST_MakeEnvelope(0,0,10,10)), ('B', ST_MakeEnvelope(10,0,20,10))")
    db.execute("CREATE INDEX zone_geom ON zones USING gist (geom)")
    db.execute("CREATE INDEX device_geom ON devices USING gist (geom)")
    assert db.bulk_insert("devices", ["name", "geom"], [
        ("inside", "POINT(5 5)"), ("edge", "POINT(10 5)"), ("outside", "POINT(50 50)")
    ]) == 3
    join = "SELECT d.name AS device, z.code AS zone FROM devices d JOIN zones z ON ST_Contains(z.geom,d.geom)"
    assert db.execute(join).rows == [{"device": "inside", "zone": "A"}]
    db.execute("UPDATE devices SET geom = ST_Point($1,$2) WHERE name = $3", [15, 5, "outside"])
    assert sorted(db.execute(join).rows, key=lambda r: r["device"]) == [
        {"device": "inside", "zone": "A"}, {"device": "outside", "zone": "B"}
    ]
    db.execute("DELETE FROM devices WHERE name = 'inside'")
    assert db.execute("SELECT name FROM devices WHERE ST_DWithin(geom,ST_Point(15,5),0)").rows == [{"name": "outside"}]


def test_geometry_persists_with_schema_and_srid(tmp_path):
    path = str(tmp_path / "geometry")
    with NexusDB(path) as db:
        db.execute("CREATE TABLE points (geom GEOMETRY)")
        db.execute("CREATE INDEX points_geom ON points USING gist (geom)")
        db.execute("INSERT INTO points VALUES (ST_GeomFromText($1,$2))", ["POINT(1.25 -2.5)", 3857])
    with NexusDB(path) as db:
        assert db.execute("SELECT geom, ST_SRID(geom) AS srid FROM points").rows == [{"geom": "POINT(1.25 -2.5)", "srid": 3857}]
        with pytest.raises(RuntimeError, match="[Ww][Kk][Tt]|[Gg]eometry|[Ii]nvalid"):
            db.execute("INSERT INTO points VALUES ($1)", ["not geometry"])


def test_null_geometry(db):
    assert db.execute("SELECT ST_AsText($1) AS geom", [None]).rows == [{"geom": None}]
    assert db.execute("SELECT ST_DWithin(ST_Point(0,0),ST_Point(0,0),$1) AS near", [-1]).rows == [{"near": False}]


def test_closed_connection_raises(db):
    db.close()
    with pytest.raises(RuntimeError, match="closed"):
        db.execute("SELECT ST_Point(0,0)")
