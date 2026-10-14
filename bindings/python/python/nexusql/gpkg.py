"""GeoPackage (.gpkg) import/export for NexusQL.

A GeoPackage is a SQLite file, so this module uses only the stdlib
`sqlite3` module to read/write the container tables (`gpkg_contents`,
`gpkg_geometry_columns`, `gpkg_spatial_ref_sys`) and feature tables, and
the pure-Python `nexusql._geom` helpers to convert GeoPackage WKB geometry
blobs to/from the WKT text that NexusQL's `GEOMETRY` columns accept.

Only 2D (no Z/M) geometry is supported, matching NexusQL's `GEOMETRY`
type. This is deliberately narrow: it is built for facility-map style
GeoPackages (KFM's `zones`/`devices`/`cameras`/`ports` layers) rather
than as a general-purpose OGC GeoPackage library.
"""

from __future__ import annotations

import re
import sqlite3
import struct
from typing import Any, Dict, Iterable, List, Optional, Sequence, Tuple

from . import _geom

__all__ = ["import_gpkg", "export_gpkg"]

_GPKG_MAGIC = b"GP"

# sqlite3 column type affinity -> NexusQL SQL type for non-geometry columns.
_SQLITE_TYPE_TO_NEXUS = {
    "INTEGER": "INT",
    "INT": "INT",
    "BIGINT": "INT",
    "REAL": "DOUBLE",
    "DOUBLE": "DOUBLE",
    "FLOAT": "DOUBLE",
    "TEXT": "TEXT",
    "VARCHAR": "TEXT",
    "BOOLEAN": "BOOLEAN",
    "BOOL": "BOOLEAN",
}


def _nexus_type_for(sqlite_decl_type: str) -> str:
    base = (sqlite_decl_type or "TEXT").split("(")[0].strip().upper()
    return _SQLITE_TYPE_TO_NEXUS.get(base, "TEXT")


_IDENT_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")


def _quote_ident(name: str) -> str:
    """Validate (not quote) an identifier.

    NexusQL's parser does not strip double quotes from a quoted
    identifier before using it in `CREATE INDEX ... USING gist (...)`
    column lookups, so a quoted `"geom"` column cannot be indexed. Until
    that is fixed on the Rust side, table/layer/column names coming out
    of a GeoPackage or GeoJSON file must already be plain SQL
    identifiers; anything else is rejected rather than silently mangled.
    """
    if not _IDENT_RE.match(name):
        raise ValueError(
            f"identifier {name!r} is not a plain SQL identifier "
            "(NexusQL's CREATE INDEX ... USING gist does not support "
            "quoted identifiers yet); rename the column/layer before importing"
        )
    return name


# --------------------------------------------------------------------------
# GeoPackage binary header (GeoPackage spec 2.1.3)
# --------------------------------------------------------------------------


def _parse_gpkg_geom_blob(blob: bytes) -> Tuple[int, bytes]:
    """Return (srs_id, wkb_bytes), stripping the GeoPackage binary header."""
    if blob[0:2] != _GPKG_MAGIC:
        raise ValueError("not a GeoPackage geometry blob (bad magic)")
    flags = blob[3]
    little_endian = bool(flags & 0x01)
    envelope_code = (flags >> 1) & 0x07
    endian = "<" if little_endian else ">"
    srs_id = struct.unpack_from(endian + "i", blob, 4)[0]
    envelope_doubles = {0: 0, 1: 4, 2: 6, 3: 6, 4: 8}.get(envelope_code, 0)
    header_len = 8 + envelope_doubles * 8
    return srs_id, blob[header_len:]


def _build_gpkg_geom_blob(srs_id: int, wkb: bytes) -> bytes:
    """Build a GeoPackage geometry blob with no envelope, little-endian."""
    flags = 0x01  # byte order = little endian, envelope = none, not empty
    header = _GPKG_MAGIC + bytes([0, flags]) + struct.pack("<i", srs_id)
    return header + wkb


# --------------------------------------------------------------------------
# Import
# --------------------------------------------------------------------------


def import_gpkg(
    db: Any,
    path: str,
    layers: Optional[Sequence[str]] = None,
) -> Dict[str, int]:
    """Import GeoPackage feature layers into NexusQL tables.

    Creates one NexusQL table per layer, named after the layer, with the
    layer's attribute columns plus a `GEOMETRY` column (same name as the
    GeoPackage geometry column, usually `geom`), and a `USING gist`
    spatial index on that column. Does not modify the source file.

    Args:
        db: an open `nexusql.NexusDB`.
        path: path to the `.gpkg` file to read.
        layers: layer (table) names to import; defaults to every
            `features`-type entry in `gpkg_contents`.

    Returns:
        Dict mapping layer name to the number of rows imported.
    """
    conn = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    try:
        conn.row_factory = sqlite3.Row
        cur = conn.cursor()

        cur.execute(
            "SELECT table_name FROM gpkg_contents WHERE data_type = 'features'"
        )
        available = [r["table_name"] for r in cur.fetchall()]
        wanted = list(layers) if layers else available
        missing = [name for name in wanted if name not in available]
        if missing:
            raise ValueError(f"layer(s) not found in {path}: {missing}")

        geom_cols: Dict[str, str] = {}
        cur.execute("SELECT table_name, column_name FROM gpkg_geometry_columns")
        for r in cur.fetchall():
            geom_cols[r["table_name"]] = r["column_name"]

        counts: Dict[str, int] = {}
        for layer in wanted:
            geom_col = geom_cols[layer]
            cur.execute(f"PRAGMA table_info({_quote_ident(layer)})")
            col_infos = cur.fetchall()  # cid, name, type, notnull, dflt, pk

            columns: List[Tuple[str, str]] = []  # (name, nexus_type)
            for info in col_infos:
                name = info["name"]
                if name == geom_col:
                    columns.append((name, "GEOMETRY"))
                else:
                    columns.append((name, _nexus_type_for(info["type"])))

            col_defs = ", ".join(
                f"{_quote_ident(n)} {t}" for n, t in columns
            )
            db.execute(f"CREATE TABLE {_quote_ident(layer)} ({col_defs})")
            db.execute(
                f"CREATE INDEX {_quote_ident(layer + '_' + geom_col + '_idx')} "
                f"ON {_quote_ident(layer)} USING gist ({_quote_ident(geom_col)})"
            )

            cur.execute(f"SELECT * FROM {_quote_ident(layer)}")
            rows = cur.fetchall()
            insert_cols = ", ".join(_quote_ident(n) for n, _ in columns)
            insert_values = []
            for i, (name, nexus_type) in enumerate(columns):
                if nexus_type == "GEOMETRY":
                    insert_values.append(f"ST_GeomFromText(${i+1})")
                else:
                    insert_values.append(f"${i+1}")
            insert_sql = (
                f"INSERT INTO {_quote_ident(layer)} ({insert_cols}) "
                f"VALUES ({', '.join(insert_values)})"
            )

            inserted = 0
            for row in rows:
                params = []
                for name, nexus_type in columns:
                    value = row[name]
                    if nexus_type == "GEOMETRY":
                        if value is None:
                            params.append(None)
                        else:
                            _srs_id, wkb = _parse_gpkg_geom_blob(value)
                            geom_type, coords = _geom.read_wkb(wkb)
                            params.append(_geom.wkt_from_geom(geom_type, coords))
                    else:
                        params.append(value)
                db.execute(insert_sql, params)
                inserted += 1
            counts[layer] = inserted
        return counts
    finally:
        conn.close()


# --------------------------------------------------------------------------
# Export
# --------------------------------------------------------------------------

# Minimal set of SRS entries GeoPackage requires/uses for facility data.
_SRS_ROWS = [
    (-1, "undefined cartesian SRS", "NONE", -1, "undefined"),
    (0, "undefined geographic SRS", "NONE", 0, "undefined"),
    (
        4326,
        "WGS 84 geodetic",
        "EPSG",
        4326,
        'GEOGCS["WGS 84",DATUM["WGS_1984",SPHEROID["WGS 84",6378137,298.257223563]],'
        'PRIMEM["Greenwich",0],UNIT["degree",0.0174532925199433],AUTHORITY["EPSG","4326"]]',
    ),
]

_CREATE_GPKG_BOILERPLATE_SQL = """
CREATE TABLE gpkg_spatial_ref_sys (
    srs_name TEXT NOT NULL,
    srs_id INTEGER NOT NULL PRIMARY KEY,
    organization TEXT NOT NULL,
    organization_coordsys_id INTEGER NOT NULL,
    definition TEXT NOT NULL,
    description TEXT
);
CREATE TABLE gpkg_contents (
    table_name TEXT NOT NULL PRIMARY KEY,
    data_type TEXT NOT NULL,
    identifier TEXT UNIQUE,
    description TEXT DEFAULT '',
    last_change DATETIME NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    min_x DOUBLE, min_y DOUBLE, max_x DOUBLE, max_y DOUBLE,
    srs_id INTEGER,
    CONSTRAINT fk_gc_r_srs_id FOREIGN KEY (srs_id) REFERENCES gpkg_spatial_ref_sys(srs_id)
);
CREATE TABLE gpkg_geometry_columns (
    table_name TEXT NOT NULL,
    column_name TEXT NOT NULL,
    geometry_type_name TEXT NOT NULL,
    srs_id INTEGER NOT NULL,
    z TINYINT NOT NULL,
    m TINYINT NOT NULL,
    CONSTRAINT pk_geom_cols PRIMARY KEY (table_name, column_name),
    CONSTRAINT fk_gc_tn FOREIGN KEY (table_name) REFERENCES gpkg_contents(table_name),
    CONSTRAINT fk_gc_srs FOREIGN KEY (srs_id) REFERENCES gpkg_spatial_ref_sys(srs_id)
);
"""


def _infer_py_type(nexus_values: Iterable[Any]) -> str:
    for v in nexus_values:
        if v is None:
            continue
        if isinstance(v, bool):
            return "BOOLEAN"
        if isinstance(v, int):
            return "INTEGER"
        if isinstance(v, float):
            return "REAL"
        return "TEXT"
    return "TEXT"


def export_gpkg(
    db: Any,
    query_or_table: str,
    path: str,
    layer: str,
    geom_column: str = "geom",
    srs_id: int = 0,
) -> int:
    """Export a NexusQL table or query result to a new GeoPackage file.

    Args:
        db: an open `nexusql.NexusDB`.
        query_or_table: either a bare table name, or a full `SELECT`
            statement (detected by a leading `SELECT`/`WITH`).
        path: destination `.gpkg` path. Overwritten if it exists.
        layer: feature table / layer name to write.
        geom_column: name of the geometry column in the result rows
            (default `"geom"`, matching NexusQL's own `ST_AsText`
            round-trip through `QueryResult.rows`).
        srs_id: `gpkg_spatial_ref_sys` id to record for the layer.

    Returns:
        Number of features written.
    """
    stripped = query_or_table.strip()
    is_query = stripped[:6].upper().startswith("SELECT") or stripped[:4].upper().startswith("WITH")
    sql = stripped if is_query else f"SELECT * FROM {_quote_ident(query_or_table)}"
    result = db.execute(sql)
    rows = list(result.rows)
    fields = list(result.fields) if rows else list(getattr(result, "fields", []) or [])
    if not fields and rows:
        fields = list(rows[0].keys())
    if geom_column not in fields and rows:
        raise ValueError(
            f"geometry column {geom_column!r} not found in result fields {fields}"
        )
    attr_fields = [f for f in fields if f != geom_column]

    import os

    if os.path.exists(path):
        os.remove(path)
    conn = sqlite3.connect(path)
    try:
        cur = conn.cursor()
        cur.executescript(_CREATE_GPKG_BOILERPLATE_SQL)
        for srs_row in _SRS_ROWS:
            cur.execute(
                "INSERT OR IGNORE INTO gpkg_spatial_ref_sys "
                "(srs_id, srs_name, organization, organization_coordsys_id, definition) "
                "VALUES (?, ?, ?, ?, ?)",
                srs_row,
            )

        geom_types = set()
        min_x = min_y = float("inf")
        max_x = max_y = float("-inf")
        parsed_rows = []
        for row in rows:
            wkt = row.get(geom_column)
            geom_type = None
            coords = None
            if wkt is not None:
                geom_type, coords = _geom.geom_from_wkt(wkt)
                geom_types.add(geom_type)
                for x, y in _iter_xy(geom_type, coords):
                    min_x, max_x = min(min_x, x), max(max_x, x)
                    min_y, max_y = min(min_y, y), max(max_y, y)
            parsed_rows.append((row, geom_type, coords))
        geometry_type_name = geom_types.pop() if len(geom_types) == 1 else "GEOMETRY"
        if min_x == float("inf"):
            min_x = min_y = max_x = max_y = 0.0

        attr_decls = []
        for f in attr_fields:
            if f == "fid":
                continue
            py_type = _infer_py_type(r[0].get(f) for r in parsed_rows)
            attr_decls.append(f"{_quote_ident(f)} {py_type}")
        has_fid = "fid" in attr_fields

        col_sql = ["fid INTEGER PRIMARY KEY AUTOINCREMENT" if not has_fid else "fid INTEGER PRIMARY KEY"]
        col_sql.append(f"{_quote_ident(geom_column)} BLOB")
        col_sql.extend(attr_decls)
        cur.execute(f"CREATE TABLE {_quote_ident(layer)} ({', '.join(col_sql)})")

        cur.execute(
            "INSERT INTO gpkg_contents "
            "(table_name, data_type, identifier, description, min_x, min_y, max_x, max_y, srs_id) "
            "VALUES (?, 'features', ?, '', ?, ?, ?, ?, ?)",
            (layer, layer, min_x, min_y, max_x, max_y, srs_id),
        )
        cur.execute(
            "INSERT INTO gpkg_geometry_columns "
            "(table_name, column_name, geometry_type_name, srs_id, z, m) "
            "VALUES (?, ?, ?, ?, 0, 0)",
            (layer, geom_column, geometry_type_name, srs_id),
        )

        insert_cols = [geom_column] + [f for f in attr_fields if f != "fid"]
        if has_fid:
            insert_cols = ["fid"] + insert_cols
        placeholders = ", ".join("?" for _ in insert_cols)
        insert_sql = (
            f"INSERT INTO {_quote_ident(layer)} ({', '.join(_quote_ident(c) for c in insert_cols)}) "
            f"VALUES ({placeholders})"
        )

        written = 0
        for row, geom_type, coords in parsed_rows:
            values = []
            for col in insert_cols:
                if col == geom_column:
                    if geom_type is None:
                        values.append(None)
                    else:
                        wkb = _geom.write_wkb(geom_type, coords)
                        values.append(_build_gpkg_geom_blob(srs_id, wkb))
                else:
                    values.append(row.get(col))
            cur.execute(insert_sql, values)
            written += 1

        conn.commit()
        return written
    finally:
        conn.close()


def _iter_xy(geom_type: str, coords: Any):
    name = geom_type.upper()
    if name == "POINT":
        yield coords
    elif name in ("LINESTRING", "MULTIPOINT"):
        yield from coords
    elif name in ("POLYGON", "MULTILINESTRING"):
        for ring in coords:
            yield from ring
    elif name == "MULTIPOLYGON":
        for poly in coords:
            for ring in poly:
                yield from ring
