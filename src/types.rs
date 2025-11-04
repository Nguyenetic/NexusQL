//! Core data types for NexusSQL
//! PROPRIETARY

use serde::{Deserialize, Serialize};

/// A single value in the database
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Value {
    Null,
    Integer(i64),
    #[serde(serialize_with = "serialize_f64", deserialize_with = "deserialize_f64")]
    Float(ordered_float::OrderedFloat<f64>),
    Text(String),
    Blob(Vec<u8>),
    Boolean(bool),
    /// Vector type for semantic search and embeddings
    /// Stored as f32 for efficiency (same as pgvector default)
    Vector(Vec<f32>),
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
            Value::Vector(v) => write!(
                f,
                "[{}]",
                v.iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}

impl Value {
    /// Calculate L2 (Euclidean) distance between two vectors
    /// Distance = sqrt(sum((a[i] - b[i])^2))
    pub fn l2_distance(&self, other: &Self) -> Option<f32> {
        match (self, other) {
            (Value::Vector(a), Value::Vector(b)) if a.len() == b.len() => {
                let sum: f32 = a.iter().zip(b.iter()).map(|(x, y)| (x - y).powi(2)).sum();
                Some(sum.sqrt())
            }
            _ => None,
        }
    }

    /// Calculate cosine distance between two vectors
    /// Distance = 1 - cosine_similarity
    /// Cosine similarity = dot_product / (norm_a * norm_b)
    pub fn cosine_distance(&self, other: &Self) -> Option<f32> {
        match (self, other) {
            (Value::Vector(a), Value::Vector(b)) if a.len() == b.len() => {
                let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
                let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
                let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

                if norm_a == 0.0 || norm_b == 0.0 {
                    return Some(1.0); // Maximum distance
                }

                let cosine_sim = dot_product / (norm_a * norm_b);
                Some(1.0 - cosine_sim)
            }
            _ => None,
        }
    }

    /// Calculate inner product (negative for distance-based sorting)
    /// Used for normalized vectors where higher values = more similar
    pub fn inner_product(&self, other: &Self) -> Option<f32> {
        match (self, other) {
            (Value::Vector(a), Value::Vector(b)) if a.len() == b.len() => {
                let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
                Some(-dot_product) // Negative so ORDER BY works correctly
            }
            _ => None,
        }
    }
}

impl Eq for Value {}

impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Discriminant for the variant
        std::mem::discriminant(self).hash(state);

        match self {
            Value::Null => {}
            Value::Integer(i) => i.hash(state),
            Value::Float(f) => f.hash(state),
            Value::Text(s) => s.hash(state),
            Value::Blob(b) => b.hash(state),
            Value::Boolean(b) => b.hash(state),
            Value::Vector(v) => {
                // Hash vectors by converting f32 to bits
                v.len().hash(state);
                for f in v {
                    f.to_bits().hash(state);
                }
            }
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
