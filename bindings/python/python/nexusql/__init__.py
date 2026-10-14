"""
NexusQL Python bindings.

Wraps the compiled `_nexusql` native module (PyO3 over the Rust NexusQL
crate) and re-exports a Pythonic surface. Build the native module with
`maturin develop` from this directory.

Example:
    >>> from nexusql import NexusDB
    >>> db = NexusDB('./mydb')
    >>> db.execute('CREATE TABLE users (id INT, name TEXT)')
    >>> db.execute('INSERT INTO users VALUES ($1, $2)', [1, 'Alice'])
    >>> result = db.execute('SELECT * FROM users')
    >>> for row in result.rows:
    ...     print(row['name'])
    >>> db.close()
"""

from __future__ import annotations

from contextlib import contextmanager
from typing import Any, Iterator, List, Optional, Sequence

try:
    from ._nexusql import NexusDB as _NativeNexusDB  # when installed as wheel
    from ._nexusql import QueryResult as _NativeQueryResult
except ImportError:
    # Fallback import path for `maturin develop` layouts that drop the
    # extension alongside this module.
    from _nexusql import NexusDB as _NativeNexusDB  # type: ignore
    from _nexusql import QueryResult as _NativeQueryResult  # type: ignore

__all__ = ["NexusDB", "QueryResult", "PreparedStatement", "connect"]


class QueryResult:
    """Result of a query execution.

    Attributes:
        rows: List of dict rows, keyed by column name (empty for DDL/DML).
        row_count: Number of rows in `rows` (equivalent to `len(rows)`).
        fields: List of column names from the first row (empty for DDL/DML).
        rows_affected: For INSERT/UPDATE/DELETE, the number of rows touched.
    """

    __slots__ = ("_inner",)

    def __init__(self, inner: _NativeQueryResult) -> None:
        self._inner = inner

    @property
    def rows(self) -> List[dict]:
        return self._inner.rows

    @property
    def row_count(self) -> int:
        return self._inner.row_count

    @property
    def fields(self) -> List[str]:
        return self._inner.fields

    @property
    def rows_affected(self) -> int:
        return self._inner.rows_affected

    def __iter__(self) -> Iterator[dict]:
        return iter(self.rows)

    def __len__(self) -> int:
        return self.row_count

    def __repr__(self) -> str:
        return f"QueryResult(row_count={self.row_count}, rows_affected={self.rows_affected})"


class NexusDB:
    """Embedded NexusQL database.

    Args:
        db_path: Path to the database directory (created if missing).

    Example:
        >>> db = NexusDB('./mydb')
        >>> db.execute('CREATE TABLE users (id INT, name TEXT)')
        >>> result = db.query('SELECT * FROM users')
        >>> db.close()
    """

    __slots__ = ("_native", "_closed")

    def __init__(self, db_path: str) -> None:
        self._native = _NativeNexusDB(db_path)
        self._closed = False

    def execute(
        self,
        sql: str,
        params: Optional[Sequence[Any]] = None,
    ) -> QueryResult:
        """Execute a SQL statement. Accepts optional positional parameters.

        Returns:
            QueryResult. For SELECT the rows attribute has the result rows;
            for DDL/DML rows_affected is populated.
        """
        self._check_open()
        if params is None:
            native = self._native.execute(sql)
        else:
            native = self._native.execute(sql, list(params))
        return QueryResult(native)

    def query(self, sql: str, params: Optional[Sequence[Any]] = None) -> QueryResult:
        """Alias for execute() retained for API compatibility."""
        return self.execute(sql, params)

    def changes(self, feed: str, since: Optional[int] = None) -> List[dict]:
        """Read committed events strictly after a cursor; None reads retained history.

        An expired cursor raises RuntimeError. Geometry images contain WKT.
        """
        self._check_open()
        return self._native.changes(feed, since)

    def watch_changes(self, feed: str, since: Optional[int] = None) -> Iterator[dict]:
        """Blocking iterator. None watches future commits; a cursor catches up first.

        The returned iterator also has recv(timeout=None), returning None on a
        timeout (seconds). Waiting releases the GIL. Slow consumers can expire.
        """
        self._check_open()
        return self._native.watch_changes(feed, since)

    def set_changefeed_retention(self, max_events: int) -> None:
        """Set the durable global event limit (positive; default 10,000)."""
        self._check_open()
        self._native.set_changefeed_retention(max_events)

    def execute_many(self, sql: str, params_list: Sequence[Sequence[Any]]) -> int:
        """Execute a statement once per parameter tuple. Returns total rows_affected."""
        self._check_open()
        total = 0
        for p in params_list:
            total += self.execute(sql, p).rows_affected
        return total

    @contextmanager
    def transaction(self) -> Iterator["NexusDB"]:
        """Context manager for transactional scope.

        Issues BEGIN / COMMIT / ROLLBACK via SQL. Rolls back on exception.
        """
        self.execute("BEGIN")
        try:
            yield self
            self.execute("COMMIT")
        except Exception:
            try:
                self.execute("ROLLBACK")
            except Exception:
                pass  # rollback best-effort; re-raise original
            raise

    def prepare(self, sql: str) -> "PreparedStatement":
        """Return a PreparedStatement wrapper.

        Current implementation re-sends the SQL on each execute; once
        NexusSQL exposes a native prepare/bind API this will cache the
        parse.
        """
        return PreparedStatement(self, sql)

    def set_bulk_mode(self, enabled: bool) -> None:
        """Toggle bulk-load mode.

        When ON, subsequent DDL/DML skips per-statement disk persistence
        (~200x faster on bulk loads). Tables touched in bulk mode are only
        durable after `flush()` / `close()` / `set_bulk_mode(False)`.
        """
        self._check_open()
        self._native.set_bulk_mode(enabled)

    def bulk_insert(
        self,
        table: str,
        columns: Sequence[str],
        rows: Sequence[Sequence[Any]],
    ) -> int:
        """Directly bulk-insert rows into `table`. Bypasses SQL parser.

        Args:
            table: destination table (must already exist via CREATE TABLE).
            columns: ordered list of column names that `rows` provides.
                     Other columns default to NULL.
            rows: iterable of row tuples/lists. Types follow the execute()
                  parameter mapping (None/bool/int/float/str/bytes/list[float]).

        Returns:
            Number of rows inserted.

        Example:
            db.execute("CREATE TABLE t (id INT, name TEXT)")
            db.bulk_insert(
                "t",
                ["id", "name"],
                [(1, "a"), (2, "b"), (3, "c")],
            )

        This skips sqlparser-rs + optimizer + router overhead that every
        `execute(INSERT ...)` incurs — typically 5-50x faster for wide bulk
        loads.
        """
        self._check_open()
        return self._native.bulk_insert(table, list(columns), rows)

    def flush(self) -> None:
        """Flush any deferred writes to disk. Idempotent."""
        self._check_open()
        self._native.flush()

    @contextmanager
    def bulk_load(self) -> Iterator["NexusDB"]:
        """Context-manager for bulk loads. Enters bulk mode, flushes on exit.

        Example:
            with db.bulk_load():
                for stmt in many_statements:
                    db.execute(stmt)
        """
        self.set_bulk_mode(True)
        try:
            yield self
        finally:
            try:
                self.flush()
            finally:
                self.set_bulk_mode(False)

    def close(self) -> None:
        """Close the database. Safe to call multiple times."""
        if not self._closed:
            # Best-effort flush in case bulk mode was left on
            try:
                self._native.flush()
            except Exception:
                pass
            self._native.close()
            self._closed = True

    def is_open(self) -> bool:
        return not self._closed and self._native.is_open()

    def __enter__(self) -> "NexusDB":
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.close()

    def __repr__(self) -> str:
        return "NexusDB(open)" if self.is_open() else "NexusDB(closed)"

    def _check_open(self) -> None:
        if self._closed:
            raise RuntimeError("NexusDB is closed")


class PreparedStatement:
    """Thin wrapper over a SQL string bound to a NexusDB instance."""

    __slots__ = ("_db", "_sql")

    def __init__(self, db: NexusDB, sql: str) -> None:
        self._db = db
        self._sql = sql

    def execute(self, *params: Any) -> QueryResult:
        return self._db.execute(self._sql, params if params else None)

    def close(self) -> None:
        # No native resource to release until NexusQL exposes prepared handles
        pass


def connect(db_path: str) -> NexusDB:
    """Convenience alias matching DB-API 2.0 naming."""
    return NexusDB(db_path)
