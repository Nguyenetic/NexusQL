"""Regenerate tests/fixtures/kfm_subset.gpkg from a real KFM GeoPackage.

One-off generator, not run by pytest. The committed `kfm_subset.gpkg` is a
small, deterministic copy of a handful of rows from KFM's
`dist/kfm.gpkg` (zones polygons + devices/cameras points), so the test
suite does not depend on that path existing. Re-run manually if the
fixture needs to be refreshed from a newer KFM export:

    python tests/fixtures/make_fixture.py <path-to-kfm.gpkg>
"""

from __future__ import annotations

import sqlite3
import sys
from pathlib import Path

FIXTURE_PATH = Path(__file__).parent / "kfm_subset.gpkg"

# (layer, number of rows to copy, order_by)
LAYERS = [
    ("zones", 5, "fid"),
    ("devices", 8, "fid"),
    ("cameras", 4, "fid"),
]


def main(source: str) -> None:
    if FIXTURE_PATH.exists():
        FIXTURE_PATH.unlink()

    src = sqlite3.connect(f"file:{source}?mode=ro", uri=True)
    src.row_factory = sqlite3.Row
    dst = sqlite3.connect(str(FIXTURE_PATH))

    scur = src.cursor()
    dcur = dst.cursor()

    dcur.execute(
        "CREATE TABLE gpkg_spatial_ref_sys ("
        "srs_name TEXT NOT NULL, srs_id INTEGER NOT NULL PRIMARY KEY, "
        "organization TEXT NOT NULL, organization_coordsys_id INTEGER NOT NULL, "
        "definition TEXT NOT NULL, description TEXT)"
    )
    dcur.execute(
        "CREATE TABLE gpkg_contents ("
        "table_name TEXT NOT NULL PRIMARY KEY, data_type TEXT NOT NULL, "
        "identifier TEXT UNIQUE, description TEXT DEFAULT '', "
        "last_change DATETIME NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')), "
        "min_x DOUBLE, min_y DOUBLE, max_x DOUBLE, max_y DOUBLE, srs_id INTEGER)"
    )
    dcur.execute(
        "CREATE TABLE gpkg_geometry_columns ("
        "table_name TEXT NOT NULL, column_name TEXT NOT NULL, "
        "geometry_type_name TEXT NOT NULL, srs_id INTEGER NOT NULL, "
        "z TINYINT NOT NULL, m TINYINT NOT NULL, "
        "PRIMARY KEY (table_name, column_name))"
    )

    scur.execute("SELECT * FROM gpkg_spatial_ref_sys")
    srs_rows = scur.fetchall()
    for r in srs_rows:
        dcur.execute(
            "INSERT INTO gpkg_spatial_ref_sys VALUES (?, ?, ?, ?, ?, ?)",
            tuple(r),
        )

    for layer, limit, order_by in LAYERS:
        scur.execute(f"PRAGMA table_info({layer})")
        cols = scur.fetchall()
        col_decls = []
        for c in cols:
            pk = " PRIMARY KEY" if c["pk"] else ""
            col_decls.append(f'"{c["name"]}" {c["type"]}{pk}')
        dcur.execute(f'CREATE TABLE "{layer}" ({", ".join(col_decls)})')

        scur.execute("SELECT * FROM gpkg_contents WHERE table_name = ?", (layer,))
        contents_row = scur.fetchone()
        scur.execute(
            "SELECT * FROM gpkg_geometry_columns WHERE table_name = ?", (layer,)
        )
        geom_col_row = scur.fetchone()

        col_names = [c["name"] for c in cols]
        placeholders = ", ".join("?" for _ in col_names)
        scur.execute(f'SELECT * FROM "{layer}" ORDER BY {order_by} LIMIT ?', (limit,))
        rows = scur.fetchall()
        min_x = min_y = float("inf")
        max_x = max_y = float("-inf")
        for row in rows:
            dcur.execute(
                f'INSERT INTO "{layer}" ({", ".join(col_names)}) VALUES ({placeholders})',
                tuple(row),
            )

        dcur.execute(
            "INSERT INTO gpkg_contents VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (
                contents_row["table_name"],
                contents_row["data_type"],
                contents_row["identifier"],
                contents_row["description"],
                contents_row["last_change"],
                contents_row["min_x"],
                contents_row["min_y"],
                contents_row["max_x"],
                contents_row["max_y"],
                contents_row["srs_id"],
            ),
        )
        dcur.execute(
            "INSERT INTO gpkg_geometry_columns VALUES (?, ?, ?, ?, ?, ?)",
            tuple(geom_col_row),
        )
        print(f"{layer}: copied {len(rows)} rows")

    dst.commit()
    dst.close()
    src.close()
    print(f"wrote {FIXTURE_PATH}")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else r"C:\Users\ee01287\Documents\Projects\KFM\dist\kfm.gpkg")
