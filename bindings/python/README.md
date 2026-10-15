# NexusQL Python binding

The PyO3 binding exposes `NexusDB` and `QueryResult`. Geometry values are returned
as WKT strings; ordinary SQL values become Python `None`, `int`, `float`, `str`,
`bytes`, `bool`, or lists of floats.

## Build and test from source

Run these commands in `bindings/python`, using a Rust toolchain and Python 3.8+
(the build/test run for this change used Rust 1.98 and Python 3.12 on Windows):

```sh
python -m venv .venv
# Windows: .venv\Scripts\activate
# Unix: source .venv/bin/activate
python -m pip install maturin pytest
maturin develop
pytest -q
```

`maturin develop` installs the compiled module into the active virtual environment.
Use `maturin build --release` to produce a wheel for distribution.

## Geometry and parameters

```python
from nexusql import NexusDB

with NexusDB(":memory:") as db:
    db.execute("CREATE TABLE devices (name TEXT, geom GEOMETRY)")
    db.execute("CREATE INDEX devices_geom ON devices USING gist (geom)")
    db.execute(
        "INSERT INTO devices VALUES ($1, ST_GeomFromText($2))",
        ("CAM-01", "POINT(3 4)"),
    )
    result = db.execute(
        "SELECT name, geom FROM devices WHERE ST_DWithin(geom, ST_Point($1,$2), $3)",
        [0, 0, 10],
    )
    assert result.rows == [{"name": "CAM-01", "geom": "POINT(3 4)"}]
```

Pass WKT directly to a GEOMETRY column or through `ST_GeomFromText`. Use `$1`,
`$2`, etc. for positional parameters; the wrapper accepts lists or tuples.
`ST_AsText` and Python geometry output omit SRID metadata; `ST_SRID` retrieves it.
See [geometry functions, semantics and index behavior](../../docs/guides/geometry.md).

## API

- `NexusDB(path)` / `connect(path)`: open a database directory or `:memory:`.
- `execute(sql, params=None)` / `query(sql, params=None)`: return `QueryResult`.
- `bulk_insert(table, columns, rows)`: insert rows directly, return the inserted
  count; WKT coercion and spatial index maintenance still apply.
- `bulk_load()`, `set_bulk_mode(enabled)`, `flush()`: defer and flush disk writes.
- `close()`, `is_open()`, and database context manager: manage connection lifetime.

`QueryResult.rows` is a list of dictionaries, `row_count` is its length, and
`fields` contains the first row's column names. `rows_affected` is the number of
rows an INSERT, UPDATE or DELETE changed (the row count for SELECT).
`bulk_insert` returns the actual count. Parameters may be written `$1, $2` or,
as with `sqlite3`, `?` (bound in order of appearance).

The wrapper also provides `execute_many`, `prepare`, and `transaction` helpers.
`prepare` currently resends SQL on each call; it does not cache a native plan.
Transaction behavior follows the core engine. Each database owns a Tokio runtime;
operations are synchronous and release the Python GIL while the engine executes.

## Full-text search

`CREATE INDEX ... USING fulltext (column)` adds a keyword index with BM25
ranking that can replace SQLite FTS5 tables. Queries use FTS5 syntax: words,
`"phrases"`, `prefix*`, `AND`/`OR`/`NOT` and parentheses.

```python
from nexusql import NexusDB

with NexusDB("./amem") as db:
    db.execute("CREATE TABLE messages (id INT, project TEXT, created TEXT, content TEXT)")
    db.create_fulltext_index("messages", "content")
    db.execute("INSERT INTO messages VALUES (?, ?, ?, ?)", [1, "nexus", "2026-10-01", "WAL recovery notes"])

    # Helper: ranked, filtered, with highlighted excerpts.
    hits = db.search(
        "messages", "wal OR checkpoint",
        select=["id", "project"],
        where="project = ? AND created >= ?", params=["nexus", "2026-10-01"],
        limit=20, snippet_tokens=12, highlight=("[", "]"),
    )
    # [{"id": 1, "project": "nexus", "score": -0.28..., "snippet": "[WAL] recovery notes"}]

    # The same in SQL:
    db.execute(
        "SELECT id, bm25(content, ?) AS score, snippet(content, ?) AS s FROM messages "
        "WHERE content MATCH ? AND project = ? ORDER BY score LIMIT 20",
        ["wal", "wal", "wal", "nexus"],
    )
```

- `create_fulltext_index(table, column, name=None)`: one TEXT column per index.
- `search(table, query, *, column="content", select=("*",), where=None,
  params=None, limit=10, snippet_tokens=16, highlight=("[", "]"),
  ellipsis="...")`: rows ordered best first, with `score` (BM25; lower is better,
  as in FTS5) and `snippet` (omitted when `snippet_tokens=None`). `where` is a SQL
  filter using `?` placeholders bound from `params`.

See [full-text search](../../docs/guides/full-text-search.md) for the query
syntax, tokenizer, ranking and durability details.

## GeoPackage and GeoJSON (`nexusql.gpkg`, `nexusql.geojson`)

Pure-Python, stdlib-only helpers (`sqlite3`/`json`) for moving feature data in
and out of NexusQL. No GDAL, shapely, or other geo dependency.

```python
from nexusql import NexusDB, gpkg, geojson

with NexusDB(":memory:") as db:
    counts = gpkg.import_gpkg(db, "facility.gpkg")          # {"zones": 161, "devices": 27, ...}
    gpkg.export_gpkg(db, "zones", "zones_only.gpkg", "zones")
    geojson.export_geojson(db, "SELECT * FROM zones WHERE plant = 'Melt'", "melt.geojson")
    geojson.import_geojson(db, "melt.geojson", "melt_zones")
```

- `import_gpkg(db, path, layers=None)` creates one NexusQL table per feature
  layer (defaults to every `features` entry in `gpkg_contents`), with a
  `GEOMETRY` column plus the layer's attribute columns, and a `USING gist`
  spatial index on the geometry column. Does not modify the source file.
- `export_gpkg(db, query_or_table, path, layer, geom_column="geom", srs_id=0)`
  writes a valid GeoPackage (`gpkg_contents`, `gpkg_geometry_columns`,
  `gpkg_spatial_ref_sys`) that QGIS can open, from a table name or a full
  `SELECT`/`WITH` query.
- `import_geojson(db, path, table, geom_column="geom")` /
  `export_geojson(db, query_or_table, path, geom_column="geom")` do the same
  for a GeoJSON `FeatureCollection` file.

Geometry conversion goes through WKT, matching how NexusQL's Python wrapper
already represents geometry (`QueryResult.rows` returns WKT strings), so
import/export round-trips exactly for `POINT`, `LINESTRING`, `POLYGON`,
`MULTIPOINT`, `MULTILINESTRING`, and `MULTIPOLYGON` (2D only, no Z/M).

**Known limitation:** column, table, and layer names must be plain SQL
identifiers (`^[A-Za-z_][A-Za-z0-9_]*$`). NexusQL's `CREATE INDEX ... USING
gist` does not currently resolve a double-quoted identifier back to its
unquoted column, so these helpers validate and reject anything that would
need quoting rather than emit an index that silently fails to apply. Rename
the source column/layer before importing if this happens.
