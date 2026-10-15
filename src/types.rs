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
    /// Planar geometry, serialized without losing coordinates or SRID metadata.
    Geometry(crate::geometry::GeometryValue),
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
            Value::Geometry(g) => write!(f, "{}", g.as_text()),
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

    // ============================================
    // Helper methods for type extraction (Issue #23)
    // ============================================

    /// Extract as i64 if this is an Integer value
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Extract as &str if this is a Text value
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Extract as bool if this is a Boolean value
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Extract as f64 if this is a Float value
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(f.0),
            _ => None,
        }
    }

    /// Extract as &[f32] if this is a Vector value
    pub fn as_vector(&self) -> Option<&[f32]> {
        match self {
            Value::Vector(v) => Some(v.as_slice()),
            _ => None,
        }
    }

    /// Extract as &[u8] if this is a Blob value
    pub fn as_blob(&self) -> Option<&[u8]> {
        match self {
            Value::Blob(b) => Some(b.as_slice()),
            _ => None,
        }
    }

    /// Check if this value is Null
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Get the type name as a string (useful for error messages)
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "Null",
            Value::Integer(_) => "Integer",
            Value::Float(_) => "Float",
            Value::Text(_) => "Text",
            Value::Blob(_) => "Blob",
            Value::Boolean(_) => "Boolean",
            Value::Vector(_) => "Vector",
            Value::Geometry(_) => "Geometry",
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
            Value::Geometry(g) => g.hash(state),
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
            (Value::Geometry(a), Value::Geometry(b)) => Some(if a == b {
                std::cmp::Ordering::Equal
            } else {
                (a.srid, a.as_text()).cmp(&(b.srid, b.as_text()))
            }),
            (Value::Boolean(a), Value::Boolean(b)) => a.partial_cmp(b),
            (Value::Null, Value::Null) => Some(std::cmp::Ordering::Equal),
            (Value::Vector(a), Value::Vector(b)) => {
                // Compare vectors lexicographically
                for (x, y) in a.iter().zip(b.iter()) {
                    match x.partial_cmp(y) {
                        Some(std::cmp::Ordering::Equal) => continue,
                        other => return other,
                    }
                }
                a.len().partial_cmp(&b.len())
            }
            _ => None, // Different types cannot be compared
        }
    }
}

impl Value {
    /// Sort position of the type when values of different types meet:
    /// NULL first (SQLite's ORDER BY), then booleans, numbers, text, blobs.
    fn type_rank(&self) -> u8 {
        match self {
            Value::Null => 0,
            Value::Boolean(_) => 1,
            Value::Integer(_) | Value::Float(_) => 2,
            Value::Text(_) => 3,
            Value::Blob(_) => 4,
            Value::Vector(_) => 5,
            Value::Geometry(_) => 6,
        }
    }
}

/// A total order for sorting rows (`ORDER BY`, window and FETCH order). Same-type
/// values compare as `partial_cmp`;
/// INTEGER and FLOAT compare numerically (INTEGER first on a tie, so only
/// equal values are `Equal`); other mixes order by type, NULL lowest.
/// `partial_cmp` stays partial (mixed types are unordered) because existing
/// comparisons rely on it, and slice sorts compare through `PartialOrd`: sort
/// values with `sort_by(Value::cmp)`, not `sort()` / `sort_by_cached_key`.
impl Ord for Value {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match (self, other) {
            (Value::Integer(i), Value::Float(f)) => ordered_float::OrderedFloat(*i as f64)
                .cmp(f)
                .then(Ordering::Less),
            (Value::Float(f), Value::Integer(i)) => f
                .cmp(&ordered_float::OrderedFloat(*i as f64))
                .then(Ordering::Greater),
            (Value::Blob(a), Value::Blob(b)) => a.cmp(b),
            (Value::Vector(a), Value::Vector(b)) => a
                .iter()
                .zip(b)
                .map(|(x, y)| x.total_cmp(y))
                .find(|o| o.is_ne())
                .unwrap_or_else(|| a.len().cmp(&b.len())),
            _ => self
                .partial_cmp(other)
                .unwrap_or_else(|| self.type_rank().cmp(&other.type_rank())),
        }
    }
}

// From<T> trait implementations for ergonomic API
impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Integer(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Value::Integer(v as i64)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Text(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Text(v.to_string())
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Boolean(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(ordered_float::OrderedFloat(v))
    }
}

impl From<Vec<f32>> for Value {
    fn from(v: Vec<f32>) -> Self {
        Value::Vector(v)
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Value::Blob(v)
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Self {
        match v {
            Some(val) => val.into(),
            None => Value::Null,
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
            .or_else(|| {
                if column.contains('.') {
                    None
                } else {
                    self.columns
                        .iter()
                        .position(|c| c.rsplit('.').next() == Some(column))
                }
            })
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

    /// No rows, reporting how many rows an INSERT/UPDATE/DELETE changed.
    pub fn affected(rows_affected: usize) -> Self {
        Self {
            rows: vec![],
            rows_affected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_i64() {
        let val: Value = 42i64.into();
        assert_eq!(val, Value::Integer(42));
    }

    #[test]
    fn test_from_i32() {
        let val: Value = 42i32.into();
        assert_eq!(val, Value::Integer(42));
    }

    #[test]
    fn test_from_string() {
        let val: Value = "hello".to_string().into();
        assert_eq!(val, Value::Text("hello".to_string()));
    }

    #[test]
    fn test_from_str() {
        let val: Value = "hello".into();
        assert_eq!(val, Value::Text("hello".to_string()));
    }

    #[test]
    fn test_from_bool() {
        let val_true: Value = true.into();
        let val_false: Value = false.into();
        assert_eq!(val_true, Value::Boolean(true));
        assert_eq!(val_false, Value::Boolean(false));
    }

    #[test]
    fn test_from_f64() {
        let val: Value = 2.5f64.into();
        assert_eq!(val, Value::Float(ordered_float::OrderedFloat(2.5)));
    }

    #[test]
    fn test_from_vec_f32() {
        let vec = vec![1.0f32, 2.0, 3.0];
        let val: Value = vec.clone().into();
        assert_eq!(val, Value::Vector(vec));
    }

    #[test]
    fn test_from_vec_u8() {
        let blob = vec![1u8, 2, 3, 4];
        let val: Value = blob.clone().into();
        assert_eq!(val, Value::Blob(blob));
    }

    #[test]
    fn test_from_option_some() {
        let val: Value = Some(42i64).into();
        assert_eq!(val, Value::Integer(42));

        let val: Value = Some("hello").into();
        assert_eq!(val, Value::Text("hello".to_string()));

        let val: Value = Some(true).into();
        assert_eq!(val, Value::Boolean(true));
    }

    #[test]
    fn test_from_option_none() {
        let val: Value = Option::<i64>::None.into();
        assert_eq!(val, Value::Null);

        let val: Value = Option::<String>::None.into();
        assert_eq!(val, Value::Null);

        let val: Value = Option::<bool>::None.into();
        assert_eq!(val, Value::Null);
    }

    #[test]
    fn test_ergonomic_row_creation() {
        // Demonstrate ergonomic API usage
        let row = Row::new(
            vec![
                "id".to_string(),
                "name".to_string(),
                "age".to_string(),
                "active".to_string(),
            ],
            vec![1i64.into(), "Alice".into(), 30i32.into(), true.into()],
        );

        assert_eq!(row.get("id"), Some(&Value::Integer(1)));
        assert_eq!(row.get("name"), Some(&Value::Text("Alice".to_string())));
        assert_eq!(row.get("age"), Some(&Value::Integer(30)));
        assert_eq!(row.get("active"), Some(&Value::Boolean(true)));
    }

    // ============================================
    // Tests for helper methods (Issue #23)
    // ============================================

    #[test]
    fn test_as_i64() {
        assert_eq!(Value::Integer(42).as_i64(), Some(42));
        assert_eq!(Value::Text("hello".to_string()).as_i64(), None);
        assert_eq!(Value::Null.as_i64(), None);
    }

    #[test]
    fn test_as_str() {
        assert_eq!(Value::Text("hello".to_string()).as_str(), Some("hello"));
        assert_eq!(Value::Integer(42).as_str(), None);
        assert_eq!(Value::Null.as_str(), None);
    }

    #[test]
    fn test_as_bool() {
        assert_eq!(Value::Boolean(true).as_bool(), Some(true));
        assert_eq!(Value::Boolean(false).as_bool(), Some(false));
        assert_eq!(Value::Integer(1).as_bool(), None);
        assert_eq!(Value::Null.as_bool(), None);
    }

    #[test]
    fn test_as_f64() {
        let val = Value::Float(ordered_float::OrderedFloat(2.5));
        assert_eq!(val.as_f64(), Some(2.5));
        assert_eq!(Value::Integer(42).as_f64(), None);
        assert_eq!(Value::Null.as_f64(), None);
    }

    #[test]
    fn test_as_vector() {
        let vec = vec![1.0f32, 2.0, 3.0];
        let val = Value::Vector(vec.clone());
        assert_eq!(val.as_vector(), Some(vec.as_slice()));
        assert_eq!(Value::Integer(42).as_vector(), None);
        assert_eq!(Value::Null.as_vector(), None);
    }

    #[test]
    fn test_as_blob() {
        let blob = vec![1u8, 2, 3, 4];
        let val = Value::Blob(blob.clone());
        assert_eq!(val.as_blob(), Some(blob.as_slice()));
        assert_eq!(Value::Integer(42).as_blob(), None);
        assert_eq!(Value::Null.as_blob(), None);
    }

    #[test]
    fn test_is_null() {
        assert!(Value::Null.is_null());
        assert!(!Value::Integer(42).is_null());
        assert!(!Value::Text("hello".to_string()).is_null());
        assert!(!Value::Boolean(true).is_null());
    }

    #[test]
    fn test_type_name() {
        assert_eq!(Value::Null.type_name(), "Null");
        assert_eq!(Value::Integer(42).type_name(), "Integer");
        assert_eq!(
            Value::Float(ordered_float::OrderedFloat(2.5)).type_name(),
            "Float"
        );
        assert_eq!(Value::Text("hello".to_string()).type_name(), "Text");
        assert_eq!(Value::Blob(vec![1, 2, 3]).type_name(), "Blob");
        assert_eq!(Value::Boolean(true).type_name(), "Boolean");
        assert_eq!(Value::Vector(vec![1.0, 2.0]).type_name(), "Vector");
    }

    #[test]
    fn test_helper_methods_ergonomic_usage() {
        // Demonstrate ergonomic extraction from query results
        let row = Row::new(
            vec!["id".into(), "name".into(), "active".into(), "score".into()],
            vec![
                Value::Integer(1),
                Value::Text("Alice".to_string()),
                Value::Boolean(true),
                Value::Float(ordered_float::OrderedFloat(95.5)),
            ],
        );

        // Clean extraction without pattern matching
        let id = row.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        let name = row.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let active = row.get("active").and_then(|v| v.as_bool()).unwrap_or(false);
        let score = row.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);

        assert_eq!(id, 1);
        assert_eq!(name, "Alice");
        assert!(active);
        assert!((score - 95.5).abs() < 0.001);
    }

    #[test]
    fn ord_is_total_across_types() {
        use ordered_float::OrderedFloat;
        // Mixed NULL / FLOAT keys used to compare Equal to everything, which
        // made ORDER BY panic ("does not correctly implement a total order").
        let mut keys: Vec<Value> = (0..200)
            .map(|i| match i % 5 {
                0 => Value::Null,
                1 => Value::Float(OrderedFloat(-(i as f64) / 7.0)),
                2 => Value::Integer(i % 13),
                3 => Value::Text(format!("t{}", i % 9)),
                _ => Value::Float(OrderedFloat((i % 4) as f64)),
            })
            .collect();
        keys.sort_by(Value::cmp);
        if let Some(w) = keys.windows(2).find(|w| w[0].cmp(&w[1]).is_gt()) {
            panic!("{:?} > {:?}", w[0], w[1]);
        }
        assert!(keys[..40].iter().all(|v| *v == Value::Null), "NULL first");
        assert!(matches!(keys.last(), Some(Value::Text(_))));
        let (one, one_f) = (Value::Integer(1), Value::Float(OrderedFloat(1.0)));
        assert_eq!(one.cmp(&one_f), std::cmp::Ordering::Less);
        assert_eq!(one_f.cmp(&one), std::cmp::Ordering::Greater);
        assert_eq!(
            Value::Integer(2).cmp(&Value::Float(OrderedFloat(1.5))),
            std::cmp::Ordering::Greater
        );
    }
}
