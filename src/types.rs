//! Core data types for NexusSQL
//! PROPRIETARY

use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};

/// A single value in the database
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Value {
    Null,
    Integer(i64),
    #[serde(serialize_with = "serialize_f64", deserialize_with = "deserialize_f64")]
    Float(ordered_float::OrderedFloat<f64>),
    Text(String),
    Blob(Vec<u8>),
    Boolean(bool),
}

// Helper functions for f64 serialization
fn serialize_f64<S>(
    val: &ordered_float::OrderedFloat<f64>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_f64(val.into_inner())
}

fn deserialize_f64<'de, D>(deserializer: D) -> Result<ordered_float::OrderedFloat<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val = f64::deserialize(deserializer)?;
    Ok(ordered_float::OrderedFloat(val))
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "NULL"),
            Value::Integer(i) => write!(f, "{}", i),
            Value::Float(fl) => write!(f, "{}", fl),
            Value::Text(s) => write!(f, "{}", s),
            Value::Blob(b) => write!(f, "<blob {} bytes>", b.len()),
            Value::Boolean(b) => write!(f, "{}", b),
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => a.partial_cmp(b),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
            (Value::Text(a), Value::Text(b)) => a.partial_cmp(b),
            (Value::Boolean(a), Value::Boolean(b)) => a.partial_cmp(b),
            (Value::Null, Value::Null) => Some(std::cmp::Ordering::Equal),
            _ => None, // Different types cannot be compared
        }
    }
}

/// A row of data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub columns: Vec<String>,
    pub values: Vec<Value>,
}

impl Row {
    pub fn new(columns: Vec<String>, values: Vec<Value>) -> Self {
        Self { columns, values }
    }

    pub fn get(&self, column: &str) -> Option<&Value> {
        self.columns
            .iter()
            .position(|c| c == column)
            .and_then(|i| self.values.get(i))
    }
}

/// Query result set
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultSet {
    pub rows: Vec<Row>,
    pub rows_affected: usize,
}

impl ResultSet {
    pub fn new(rows: Vec<Row>) -> Self {
        let count = rows.len();
        Self {
            rows,
            rows_affected: count,
        }
    }

    pub fn empty() -> Self {
        Self {
            rows: vec![],
            rows_affected: 0,
        }
    }
}
