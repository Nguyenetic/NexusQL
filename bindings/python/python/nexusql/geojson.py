"""GeoJSON (FeatureCollection) import/export for NexusQL.

Pure stdlib (`json`); geometry conversion goes through `nexusql._geom`,
the same WKT machinery `gpkg.py` uses, so GeoJSON and GeoPackage share
one geometry implementation.
"""

from __future__ import annotations

import json
import re
from typing import Any, Dict, Optional

from . import _geom

__all__ = ["import_geojson", "export_geojson"]

_PY_TYPE_TO_NEXUS = {
    bool: "BOOLEAN",
    int: "INT",
    float: "DOUBLE",
    str: "TEXT",
}

_IDENT_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")


def _quote_ident(name: str) -> str:
    """Validate (not quote) an identifier.

    See the matching note in `gpkg.py`: NexusQL's `CREATE INDEX ...
    USING gist` does not resolve quoted identifiers, so property/table
    names must already be plain SQL identifiers.
    """
    if not _IDENT_RE.match(name):
        raise ValueError(
            f"identifier {name!r} is not a plain SQL identifier "
            "(NexusQL's CREATE INDEX ... USING gist does not support "
            "quoted identifiers yet); rename the property/table before importing"
        )
    return name


def _infer_nexus_type(values) -> str:
    for v in values:
        if v is None:
            continue
        for py_type, nexus_type in _PY_TYPE_TO_NEXUS.items():
            if isinstance(v, py_type):
                return nexus_type
        return "TEXT"
    return "TEXT"


def import_geojson(
    db: Any,
    path: str,
    table: str,
    geom_column: str = "geom",
) -> int:
    """Import a GeoJSON FeatureCollection into a new NexusQL table.

    Creates `table` with a `GEOMETRY` column (`geom_column`) plus one
    column per key seen across all features' `properties`, and a
    `USING gist` spatial index on the geometry column.

    Returns the number of features imported.
    """
    with open(path, "r", encoding="utf-8") as f:
        data = json.load(f)
    if data.get("type") != "FeatureCollection":
        raise ValueError(f"{path} is not a GeoJSON FeatureCollection")
    features = data.get("features", [])

    prop_names: list = []
    seen = set()
    for feat in features:
        for key in (feat.get("properties") or {}).keys():
            if key not in seen:
                seen.add(key)
                prop_names.append(key)

    prop_types: Dict[str, str] = {}
    for name in prop_names:
        values = [
            (feat.get("properties") or {}).get(name) for feat in features
        ]
        prop_types[name] = _infer_nexus_type(values)

    col_defs = [f"{_quote_ident(geom_column)} GEOMETRY"]
    col_defs += [f"{_quote_ident(n)} {prop_types[n]}" for n in prop_names]
    db.execute(f"CREATE TABLE {_quote_ident(table)} ({', '.join(col_defs)})")
    db.execute(
        f"CREATE INDEX {_quote_ident(table + '_' + geom_column + '_idx')} "
        f"ON {_quote_ident(table)} USING gist ({_quote_ident(geom_column)})"
    )

    insert_cols = [geom_column] + prop_names
    placeholders = [f"ST_GeomFromText($1)"] + [f"${i+2}" for i in range(len(prop_names))]
    insert_sql = (
        f"INSERT INTO {_quote_ident(table)} "
        f"({', '.join(_quote_ident(c) for c in insert_cols)}) "
        f"VALUES ({', '.join(placeholders)})"
    )

    inserted = 0
    for feat in features:
        geometry = feat.get("geometry")
        wkt = None
        if geometry is not None:
            geom_type, coords = _geom.geom_from_geojson(geometry)
            wkt = _geom.wkt_from_geom(geom_type, coords)
        props = feat.get("properties") or {}
        params = [wkt] + [props.get(n) for n in prop_names]
        db.execute(insert_sql, params)
        inserted += 1
    return inserted


def export_geojson(
    db: Any,
    query_or_table: str,
    path: str,
    geom_column: str = "geom",
) -> int:
    """Export a NexusQL table or query result to a GeoJSON FeatureCollection file.

    `query_or_table` is either a bare table name or a full `SELECT`
    statement (detected by a leading `SELECT`/`WITH`). Geometry comes
    from `geom_column` (default `"geom"`); every other field becomes a
    GeoJSON `properties` entry.

    Returns the number of features written.
    """
    stripped = query_or_table.strip()
    is_query = stripped[:6].upper().startswith("SELECT") or stripped[:4].upper().startswith("WITH")
    sql = stripped if is_query else f"SELECT * FROM {_quote_ident(query_or_table)}"
    result = db.execute(sql)
    rows = list(result.rows)

    features = []
    for row in rows:
        wkt = row.get(geom_column)
        geometry: Optional[dict] = None
        if wkt is not None:
            geom_type, coords = _geom.geom_from_wkt(wkt)
            geometry = _geom.geojson_from_geom(geom_type, coords)
        properties = {k: v for k, v in row.items() if k != geom_column}
        features.append(
            {"type": "Feature", "geometry": geometry, "properties": properties}
        )

    collection = {"type": "FeatureCollection", "features": features}
    with open(path, "w", encoding="utf-8") as f:
        json.dump(collection, f)
    return len(features)
