//! Python bindings for NexusQL.
//!
//! Exposes a `NexusDB` Python class that wraps `nexusql::NexusSQL`, plus a
//! `QueryResult` class mirroring the existing stub API in `nexusql.py`.
//! Every `execute` call blocks the current Python thread on a per-instance
//! tokio runtime; for true async use call from an async executor via
//! `asyncio.to_thread` or equivalent.

use pyo3::exceptions::{PyRuntimeError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::sync::Arc;
use tokio::runtime::Runtime;

use nexusql::types::{Row, Value};
use nexusql::{NexusSQL, ResultSet};

fn change_to_py(py: Python<'_>, event: &nexusql::changefeed::ChangeEvent) -> PyResult<PyObject> {
    let result = PyDict::new_bound(py);
    result.set_item("feed", &event.feed)?;
    result.set_item("sequence", event.sequence)?;
    result.set_item("commit_sequence", event.commit_sequence)?;
    result.set_item("timestamp_ms", event.timestamp_ms)?;
    result.set_item("table", &event.table)?;
    result.set_item("row_id", event.row_id)?;
    result.set_item("operation", event.operation.as_str())?;
    for (key, row) in [("old", &event.old), ("new", &event.new)] {
        result.set_item(
            key,
            row.as_ref().map(|row| row_to_dict(py, row)).transpose()?,
        )?;
    }
    if let Some(zone) = &event.zone {
        let value = PyDict::new_bound(py);
        value.set_item("table", &zone.table)?;
        value.set_item("row_id", zone.row_id)?;
        value.set_item("values", row_to_dict(py, &zone.values)?)?;
        result.set_item("zone", value)?;
    } else {
        result.set_item("zone", py.None())?;
    }
    Ok(result.into())
}

/// Blocking iterator; no unbounded queue and the GIL is released while waiting.
#[pyclass]
struct ChangeIterator {
    subscription: std::sync::Mutex<nexusql::changefeed::ChangeSubscription>,
}

#[pymethods]
impl ChangeIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<PyObject> {
        self.recv(py, None)?
            .ok_or_else(|| rt_err("Changefeed closed"))
    }

    /// Return the next event, or None after timeout seconds.
    #[pyo3(signature = (timeout=None))]
    fn recv(&self, py: Python<'_>, timeout: Option<f64>) -> PyResult<Option<PyObject>> {
        let timeout = timeout
            .map(std::time::Duration::try_from_secs_f64)
            .transpose()
            .map_err(|_| {
                pyo3::exceptions::PyValueError::new_err("timeout must be finite and nonnegative")
            })?;
        let event = py
            .allow_threads(|| {
                let mut subscription = self.subscription.lock().unwrap();
                match timeout {
                    Some(timeout) => subscription.recv_timeout(timeout),
                    None => subscription.recv().map(Some),
                }
            })
            .map_err(rt_err)?;
        event
            .as_ref()
            .map(|event| change_to_py(py, event))
            .transpose()
    }
}

fn rt_err(e: impl std::fmt::Display) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

/// Convert a NexusQL `Value` to a native Python object.
fn value_to_py(py: Python<'_>, v: &Value) -> PyObject {
    match v {
        Value::Null => py.None(),
        Value::Integer(i) => i.to_object(py),
        Value::Float(f) => f.into_inner().to_object(py),
        Value::Text(s) => s.to_object(py),
        Value::Geometry(g) => g.as_text().to_object(py),
        Value::Blob(b) => pyo3::types::PyBytes::new_bound(py, b).into(),
        Value::Boolean(b) => b.to_object(py),
        Value::Vector(v) => v.to_object(py),
    }
}

/// Convert a Python value into a NexusQL parameter `Value`.
fn py_to_value(obj: &Bound<'_, PyAny>) -> PyResult<Value> {
    if obj.is_none() {
        return Ok(Value::Null);
    }
    // Order matters: bool is a subclass of int in Python, so check bool first
    if let Ok(b) = obj.extract::<bool>() {
        return Ok(Value::Boolean(b));
    }
    if let Ok(i) = obj.extract::<i64>() {
        return Ok(Value::Integer(i));
    }
    if let Ok(f) = obj.extract::<f64>() {
        return Ok(Value::Float(f.into()));
    }
    if let Ok(s) = obj.extract::<String>() {
        return Ok(Value::Text(s));
    }
    if let Ok(b) = obj.extract::<Vec<u8>>() {
        return Ok(Value::Blob(b));
    }
    if let Ok(v) = obj.extract::<Vec<f32>>() {
        return Ok(Value::Vector(v));
    }
    Err(PyTypeError::new_err(format!(
        "Unsupported parameter type for NexusQL: {}",
        obj.get_type().name()?
    )))
}

/// Convert a NexusQL `Row` to a Python dict keyed by column name.
fn row_to_dict(py: Python<'_>, row: &Row) -> PyResult<PyObject> {
    let d = PyDict::new_bound(py);
    for (col, val) in row.columns.iter().zip(row.values.iter()) {
        d.set_item(col, value_to_py(py, val))?;
    }
    Ok(d.into())
}

/// Wrap a NexusQL `ResultSet` into the Python-visible `QueryResult`.
#[pyclass]
struct QueryResult {
    #[pyo3(get)]
    rows: PyObject,
    #[pyo3(get)]
    row_count: usize,
    #[pyo3(get)]
    fields: PyObject,
    #[pyo3(get)]
    rows_affected: usize,
}

#[pymethods]
impl QueryResult {
    fn __repr__(&self) -> String {
        format!(
            "QueryResult(row_count={}, rows_affected={})",
            self.row_count, self.rows_affected
        )
    }
}

fn result_to_py(py: Python<'_>, rs: ResultSet) -> PyResult<Py<QueryResult>> {
    let rows_list = PyList::empty_bound(py);
    for r in &rs.rows {
        rows_list.append(row_to_dict(py, r)?)?;
    }
    let fields = rs
        .rows
        .first()
        .map(|r| r.columns.clone())
        .unwrap_or_default();
    let fields_obj = PyList::new_bound(py, &fields).into();
    Py::new(
        py,
        QueryResult {
            rows: rows_list.into(),
            row_count: rs.rows.len(),
            fields: fields_obj,
            rows_affected: rs.rows_affected,
        },
    )
}

/// The embedded NexusQL database.
///
/// Example:
///     db = NexusDB("./mydb")
///     db.execute("CREATE TABLE users (id INT, name TEXT)")
///     db.execute("INSERT INTO users VALUES ($1, $2)", [1, "Alice"])
///     result = db.execute("SELECT * FROM users")
///     for row in result.rows:
///         print(row["name"])
///     db.close()
#[pyclass]
struct NexusDB {
    inner: Option<Arc<NexusSQL>>,
    rt: Option<Arc<Runtime>>,
}

#[pymethods]
impl NexusDB {
    /// Committed events strictly after since; None returns the retained history.
    #[pyo3(signature = (feed, since=None))]
    fn changes(&self, py: Python<'_>, feed: &str, since: Option<u64>) -> PyResult<PyObject> {
        let db = self
            .inner
            .as_ref()
            .ok_or_else(|| rt_err("NexusDB closed"))?;
        let events = py
            .allow_threads(|| db.changes(feed, since))
            .map_err(rt_err)?;
        let result = PyList::empty_bound(py);
        for event in &events {
            result.append(change_to_py(py, event)?)?;
        }
        Ok(result.into())
    }

    /// None watches future events; a cursor catches up before blocking.
    #[pyo3(signature = (feed, since=None))]
    fn watch_changes(&self, feed: &str, since: Option<u64>) -> PyResult<ChangeIterator> {
        let db = self
            .inner
            .as_ref()
            .ok_or_else(|| rt_err("NexusDB closed"))?;
        Ok(ChangeIterator {
            subscription: std::sync::Mutex::new(db.subscribe(feed, since).map_err(rt_err)?),
        })
    }

    fn set_changefeed_retention(&self, py: Python<'_>, max_events: usize) -> PyResult<()> {
        let db = self
            .inner
            .as_ref()
            .ok_or_else(|| rt_err("NexusDB closed"))?;
        py.allow_threads(|| db.set_changefeed_retention(max_events))
            .map_err(rt_err)
    }

    #[new]
    fn new(db_path: &str) -> PyResult<Self> {
        let rt = Runtime::new().map_err(rt_err)?;
        let db = rt.block_on(NexusSQL::new(db_path)).map_err(rt_err)?;
        Ok(Self {
            inner: Some(Arc::new(db)),
            rt: Some(Arc::new(rt)),
        })
    }

    /// Execute a SQL statement. For SELECT, returns `QueryResult`. For DDL/DML,
    /// also returns `QueryResult` with `rows=[]` and `rows_affected` set.
    #[pyo3(signature = (sql, params=None))]
    fn execute(
        &self,
        py: Python<'_>,
        sql: &str,
        params: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<QueryResult>> {
        let db = self
            .inner
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("NexusDB closed"))?
            .clone();
        let rt = self
            .rt
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("NexusDB closed"))?
            .clone();
        let sql = sql.to_string();

        let param_vec = if let Some(p) = params {
            let list = p
                .downcast::<PyList>()
                .map_err(|_| PyTypeError::new_err("params must be a list or None"))?;
            let mut out = Vec::with_capacity(list.len());
            for item in list.iter() {
                out.push(py_to_value(&item)?);
            }
            Some(out)
        } else {
            None
        };

        // Release the GIL while blocking on the tokio runtime.
        let rs = py.allow_threads(move || {
            rt.block_on(async move {
                match param_vec {
                    Some(params) => db.execute_with_params(&sql, Some(params)).await,
                    None => db.execute(&sql).await,
                }
            })
        });
        let rs = rs.map_err(rt_err)?;
        result_to_py(py, rs)
    }

    /// Return True if the DB is still open.
    fn is_open(&self) -> bool {
        self.inner.is_some()
    }

    /// Toggle bulk-load mode. When ON, subsequent DDL/DML skips per-statement
    /// disk persistence — state lives in memory until `flush()` or `close()`.
    /// Typical usage:
    ///     db.set_bulk_mode(True)
    ///     for stmt in ...:
    ///         db.execute(stmt)
    ///     db.flush()
    ///     db.set_bulk_mode(False)
    fn set_bulk_mode(&self, py: Python<'_>, enabled: bool) -> PyResult<()> {
        let db = self
            .inner
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("NexusDB closed"))?
            .clone();
        let rt = self
            .rt
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("NexusDB closed"))?
            .clone();
        py.allow_threads(move || rt.block_on(async move { db.set_bulk_mode(enabled).await }));
        Ok(())
    }

    /// Direct bulk insert — bypasses the SQL parser entirely.
    ///
    /// Args:
    ///     table: destination table name (must already exist).
    ///     columns: list of column names to fill. Unmentioned columns
    ///              default to NULL.
    ///     rows: list of row tuples/lists of Python values. Types follow
    ///           the same mapping as execute()'s params.
    ///
    /// Returns the number of rows inserted. For large loads this is 5-50x
    /// faster than issuing INSERT statements because it skips sqlparser,
    /// the optimizer pass, and the router.
    fn bulk_insert(
        &self,
        py: Python<'_>,
        table: &str,
        columns: Vec<String>,
        rows: &Bound<'_, PyList>,
    ) -> PyResult<usize> {
        let db = self
            .inner
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("NexusDB closed"))?
            .clone();
        let rt = self
            .rt
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("NexusDB closed"))?
            .clone();

        // Flatten rows (list[list|tuple]) -> Vec<Value> row-major.
        let ncols = columns.len();
        let row_count = rows.len();
        let mut flat: Vec<Value> = Vec::with_capacity(row_count * ncols);

        for row_any in rows.iter() {
            let row_list = row_any.downcast::<PyList>().ok();
            let row_tuple = row_any.downcast::<pyo3::types::PyTuple>().ok();
            let len = if let Some(l) = row_list.as_ref() {
                l.len()
            } else if let Some(t) = row_tuple.as_ref() {
                t.len()
            } else {
                return Err(PyTypeError::new_err("each row must be a list or tuple"));
            };
            for i in 0..ncols {
                if i < len {
                    let cell = if let Some(l) = row_list.as_ref() {
                        l.get_item(i)?
                    } else {
                        row_tuple.as_ref().unwrap().get_item(i)?
                    };
                    flat.push(py_to_value(&cell)?);
                } else {
                    flat.push(Value::Null);
                }
            }
        }

        let table = table.to_string();
        let n = py.allow_threads(move || {
            rt.block_on(async move { db.bulk_insert(&table, &columns, flat, row_count).await })
        });
        n.map_err(rt_err)
    }

    /// Flush any deferred writes to disk. Idempotent.
    fn flush(&self, py: Python<'_>) -> PyResult<()> {
        let db = self
            .inner
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("NexusDB closed"))?
            .clone();
        let rt = self
            .rt
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("NexusDB closed"))?
            .clone();
        py.allow_threads(move || rt.block_on(async move { db.flush().await }))
            .map_err(rt_err)?;
        Ok(())
    }

    /// Close the database. The underlying runtime is dropped; subsequent
    /// execute() calls raise RuntimeError("NexusDB closed").
    fn close(&mut self) -> PyResult<()> {
        // Drop Arcs — NexusSQL will shut down via its Drop if exclusively held.
        self.inner = None;
        self.rt = None;
        Ok(())
    }

    fn __repr__(&self) -> String {
        if self.inner.is_some() {
            "NexusDB(open)".to_string()
        } else {
            "NexusDB(closed)".to_string()
        }
    }
}

/// Module init.
#[pymodule]
fn _nexusql(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<NexusDB>()?;
    m.add_class::<QueryResult>()?;
    m.add_class::<ChangeIterator>()?;
    Ok(())
}
