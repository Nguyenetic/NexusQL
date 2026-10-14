"""Parity test: NexusQL vs DuckDB spatial on the same GeoPackage fixture.

`duckdb` (with its `spatial` extension) is a test-only dependency used
purely as an independent oracle: it reads the same
tests/fixtures/kfm_subset.gpkg file NexusQL imports via
`nexusql.gpkg.import_gpkg`, and this test asserts the two engines agree
on device-to-zone containment, device-pair distances, and zone areas.
Any disagreement is a real finding, not something this test should be
bent to accommodate.
"""

import math
import os

import pytest

from nexusql import NexusDB, gpkg

duckdb = pytest.importorskip("duckdb")

FIXTURE = os.path.join(os.path.dirname(__file__), "fixtures", "kfm_subset.gpkg")
TOLERANCE = 1e-9


@pytest.fixture(scope="module")
def duck():
    con = duckdb.connect()
    con.execute("INSTALL spatial")
    con.execute("LOAD spatial")
    con.execute(
        f"CREATE TABLE zones AS SELECT * FROM ST_Read('{FIXTURE}', layer='zones')"
    )
    con.execute(
        f"CREATE TABLE devices AS SELECT * FROM ST_Read('{FIXTURE}', layer='devices')"
    )
    yield con
    con.close()


@pytest.fixture
def nexus():
    with NexusDB(":memory:") as db:
        gpkg.import_gpkg(db, FIXTURE, layers=["zones", "devices"])
        yield db


def test_device_to_zone_containment_matches(duck, nexus):
    duck_rows = duck.execute(
        """
        SELECT d.fid AS device_fid, z.fid AS zone_fid
        FROM devices d JOIN zones z ON ST_Contains(z.geom, d.geom)
        ORDER BY d.fid, z.fid
        """
    ).fetchall()
    nexus_rows = nexus.execute(
        """
        SELECT d.fid AS device_fid, z.fid AS zone_fid
        FROM devices d JOIN zones z ON ST_Contains(z.geom, d.geom)
        ORDER BY d.fid, z.fid
        """
    ).rows
    nexus_pairs = [(r["device_fid"], r["zone_fid"]) for r in nexus_rows]
    assert nexus_pairs == duck_rows


def test_device_pairs_within_distance_match(duck, nexus):
    threshold = 0.0005  # degrees; fixture is in lon/lat
    duck_rows = duck.execute(
        """
        SELECT a.fid, b.fid
        FROM devices a JOIN devices b ON a.fid < b.fid
        WHERE ST_Distance(a.geom, b.geom) <= ?
        ORDER BY a.fid, b.fid
        """,
        [threshold],
    ).fetchall()
    nexus_rows = nexus.execute(
        """
        SELECT a.fid AS a_fid, b.fid AS b_fid
        FROM devices a JOIN devices b ON a.fid < b.fid
        WHERE ST_DWithin(a.geom, b.geom, $1)
        ORDER BY a.fid, b.fid
        """,
        [threshold],
    ).rows
    nexus_pairs = [(r["a_fid"], r["b_fid"]) for r in nexus_rows]
    assert nexus_pairs == duck_rows


def test_zone_areas_match_within_tolerance(duck, nexus):
    duck_rows = duck.execute(
        "SELECT fid, ST_Area(geom) AS area FROM zones ORDER BY fid"
    ).fetchall()
    nexus_rows = nexus.execute(
        "SELECT fid, ST_Area(geom) AS area FROM zones ORDER BY fid"
    ).rows

    assert len(duck_rows) == len(nexus_rows)
    mismatches = []
    for (fid, duck_area), nexus_row in zip(duck_rows, nexus_rows):
        assert fid == nexus_row["fid"]
        if not math.isclose(duck_area, nexus_row["area"], abs_tol=TOLERANCE, rel_tol=0.0):
            mismatches.append((fid, duck_area, nexus_row["area"]))
    assert not mismatches, f"zone area mismatches (fid, duckdb, nexusql): {mismatches}"
