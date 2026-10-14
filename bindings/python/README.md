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
`fields` contains the first row's column names. `rows_affected` reflects the core
engine's result; some SQL DML paths currently return zero. `bulk_insert` returns
the actual count.

The wrapper also provides `execute_many`, `prepare`, and `transaction` helpers.
`prepare` currently resends SQL on each call; it does not cache a native plan.
Transaction behavior follows the core engine. Each database owns a Tokio runtime;
operations are synchronous and release the Python GIL while the engine executes.
