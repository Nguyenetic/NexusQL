// Week 16: JSON/JSONB & Arrays
//
// Implements PostgreSQL-compatible JSON, JSONB, and Array types:
// - JSON: Text-based JSON storage
// - JSONB: Binary JSON with indexing support
// - Arrays: Typed arrays (INT[], TEXT[], etc.)
// - Operators: @>, <@, ->, ->>, ?, ?&, ?|, ||, -
// - Functions: jsonb_build_object, jsonb_agg, array_agg, unnest

use crate::types::Value;
use crate::{NexusError, Result};
use serde_json;
use std::collections::HashMap;

/// JSONB value (binary, indexed format)
#[derive(Debug, Clone, PartialEq)]
pub struct JsonbValue {
    data: serde_json::Value,
}

impl JsonbValue {
    /// Create from JSON string
    pub fn from_str(s: &str) -> Result<Self> {
        let data = serde_json::from_str(s)
            .map_err(|e| NexusError::ExecutionError(format!("Invalid JSON: {}", e)))?;
        Ok(Self { data })
    }

    /// Create from serde_json::Value
    pub fn from_value(data: serde_json::Value) -> Self {
        Self { data }
    }

    /// Get inner value
    pub fn value(&self) -> &serde_json::Value {
        &self.data
    }

    /// Convert to string
    pub fn to_string(&self) -> String {
        self.data.to_string()
    }

    /// Extract field by key (-> operator)
    pub fn extract_field(&self, key: &str) -> Option<JsonbValue> {
        self.data
            .get(key)
            .map(|v| JsonbValue::from_value(v.clone()))
    }

    /// Extract field as text (->> operator)
    pub fn extract_field_text(&self, key: &str) -> Option<String> {
        self.data.get(key).map(|v| match v {
            serde_json::Value::String(s) => s.clone(),
            _ => v.to_string(),
        })
    }

    /// Extract by path (#> operator)
    pub fn extract_path(&self, path: &[String]) -> Option<JsonbValue> {
        let mut current = &self.data;

        for element in path {
            // Try as object key
            if let Some(next) = current.get(element) {
                current = next;
                continue;
            }

            // Try as array index
            if let Ok(idx) = element.parse::<usize>() {
                if let Some(next) = current.get(idx) {
                    current = next;
                    continue;
                }
            }

            return None;
        }

        Some(JsonbValue::from_value(current.clone()))
    }

    /// Check if key exists (? operator)
    pub fn has_key(&self, key: &str) -> bool {
        if let serde_json::Value::Object(map) = &self.data {
            return map.contains_key(key);
        }
        if let serde_json::Value::Array(arr) = &self.data {
            return arr.iter().any(|v| {
                if let serde_json::Value::String(s) = v {
                    s == key
                } else {
                    false
                }
            });
        }
        false
    }

    /// Check if all keys exist (?& operator)
    pub fn has_all_keys(&self, keys: &[String]) -> bool {
        keys.iter().all(|k| self.has_key(k))
    }

    /// Check if any key exists (?| operator)
    pub fn has_any_key(&self, keys: &[String]) -> bool {
        keys.iter().any(|k| self.has_key(k))
    }

    /// Check containment (@> operator)
    pub fn contains(&self, other: &JsonbValue) -> bool {
        Self::json_contains(&self.data, &other.data)
    }

    /// Check if contained by (<@ operator)
    pub fn contained_by(&self, other: &JsonbValue) -> bool {
        other.contains(self)
    }

    /// Helper: recursive containment check
    fn json_contains(container: &serde_json::Value, contained: &serde_json::Value) -> bool {
        match (container, contained) {
            (serde_json::Value::Object(c_map), serde_json::Value::Object(cd_map)) => cd_map
                .iter()
                .all(|(k, v)| c_map.get(k).map_or(false, |cv| Self::json_contains(cv, v))),
            (serde_json::Value::Array(c_arr), serde_json::Value::Array(cd_arr)) => {
                cd_arr.iter().all(|cd_item| {
                    c_arr
                        .iter()
                        .any(|c_item| Self::json_contains(c_item, cd_item))
                })
            }
            _ => container == contained,
        }
    }

    /// Concatenate JSONB values (|| operator)
    pub fn concat(&self, other: &JsonbValue) -> JsonbValue {
        let result = match (&self.data, &other.data) {
            (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
                let mut merged = a.clone();
                for (k, v) in b {
                    merged.insert(k.clone(), v.clone());
                }
                serde_json::Value::Object(merged)
            }
            (serde_json::Value::Array(a), serde_json::Value::Array(b)) => {
                let mut merged = a.clone();
                merged.extend(b.clone());
                serde_json::Value::Array(merged)
            }
            (serde_json::Value::Array(a), v) => {
                let mut merged = a.clone();
                merged.push(v.clone());
                serde_json::Value::Array(merged)
            }
            (v, serde_json::Value::Array(b)) => {
                let mut merged = vec![v.clone()];
                merged.extend(b.clone());
                serde_json::Value::Array(merged)
            }
            _ => serde_json::Value::Array(vec![self.data.clone(), other.data.clone()]),
        };

        JsonbValue::from_value(result)
    }

    /// Delete key from object (- operator with string)
    pub fn delete_key(&self, key: &str) -> JsonbValue {
        if let serde_json::Value::Object(map) = &self.data {
            let mut new_map = map.clone();
            new_map.remove(key);
            return JsonbValue::from_value(serde_json::Value::Object(new_map));
        }
        self.clone()
    }

    /// Delete array element (- operator with index)
    pub fn delete_index(&self, idx: i64) -> JsonbValue {
        if let serde_json::Value::Array(arr) = &self.data {
            let mut new_arr = arr.clone();
            let actual_idx = if idx < 0 {
                (arr.len() as i64 + idx) as usize
            } else {
                idx as usize
            };

            if actual_idx < new_arr.len() {
                new_arr.remove(actual_idx);
            }
            return JsonbValue::from_value(serde_json::Value::Array(new_arr));
        }
        self.clone()
    }
}

/// JSONB functions
pub struct JsonbFunctions;

impl JsonbFunctions {
    /// Build JSONB object from key-value pairs
    pub fn build_object(pairs: Vec<(String, Value)>) -> Result<JsonbValue> {
        let mut map = serde_json::Map::new();

        for (key, value) in pairs {
            let json_val = Self::value_to_json(value)?;
            map.insert(key, json_val);
        }

        Ok(JsonbValue::from_value(serde_json::Value::Object(map)))
    }

    /// Build JSONB array from values
    pub fn build_array(values: Vec<Value>) -> Result<JsonbValue> {
        let arr: Result<Vec<_>> = values.into_iter().map(Self::value_to_json).collect();

        Ok(JsonbValue::from_value(serde_json::Value::Array(arr?)))
    }

    /// Convert Value to serde_json::Value
    fn value_to_json(value: Value) -> Result<serde_json::Value> {
        Ok(match value {
            Value::Integer(i) => serde_json::Value::Number(i.into()),
            Value::Float(f) => serde_json::Number::from_f64(f.into_inner())
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            Value::Text(s) => serde_json::Value::String(s),
            Value::Boolean(b) => serde_json::Value::Bool(b),
            Value::Null => serde_json::Value::Null,
            Value::Blob(_) => serde_json::Value::Null,
        })
    }

    /// Aggregate values into JSONB array (jsonb_agg)
    pub fn agg(values: Vec<Value>) -> Result<JsonbValue> {
        Self::build_array(values)
    }
}

/// Array operations
pub struct ArrayOps;

impl ArrayOps {
    /// Create array from values
    pub fn from_values(values: Vec<Value>) -> Vec<Value> {
        values
    }

    /// Unnest array into individual rows
    pub fn unnest(array: Vec<Value>) -> Vec<Value> {
        array
    }

    /// Array contains (@> operator)
    pub fn contains(array: &[Value], element: &Value) -> bool {
        array.contains(element)
    }

    /// Array overlaps (&&operator)
    pub fn overlaps(a: &[Value], b: &[Value]) -> bool {
        a.iter().any(|v| b.contains(v))
    }

    /// Array concatenation (|| operator)
    pub fn concat(a: Vec<Value>, b: Vec<Value>) -> Vec<Value> {
        let mut result = a;
        result.extend(b);
        result
    }

    /// Array length
    pub fn length(array: &[Value]) -> usize {
        array.len()
    }

    /// Aggregate values into array (array_agg)
    pub fn agg(values: Vec<Value>) -> Vec<Value> {
        values
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jsonb_from_str() {
        let jsonb = JsonbValue::from_str(r#"{"name": "Alice", "age": 30}"#).unwrap();
        assert!(jsonb.has_key("name"));
        assert!(jsonb.has_key("age"));
    }

    #[test]
    fn test_jsonb_extract_field() {
        let jsonb = JsonbValue::from_str(r#"{"user": {"name": "Alice", "age": 30}}"#).unwrap();

        let user = jsonb.extract_field("user").unwrap();
        let name = user.extract_field_text("name").unwrap();
        assert_eq!(name, "Alice");
    }

    #[test]
    fn test_jsonb_extract_path() {
        let jsonb = JsonbValue::from_str(r#"{"a": {"b": {"c": "value"}}}"#).unwrap();

        let result = jsonb
            .extract_path(&["a".to_string(), "b".to_string(), "c".to_string()])
            .unwrap();
        assert_eq!(result.to_string(), r#""value""#);
    }

    #[test]
    fn test_jsonb_contains() {
        let jsonb1 = JsonbValue::from_str(r#"{"a": 1, "b": 2, "c": 3}"#).unwrap();
        let jsonb2 = JsonbValue::from_str(r#"{"b": 2}"#).unwrap();

        assert!(jsonb1.contains(&jsonb2));
        assert!(jsonb2.contained_by(&jsonb1));
    }

    #[test]
    fn test_jsonb_has_keys() {
        let jsonb = JsonbValue::from_str(r#"{"a": 1, "b": 2, "c": 3}"#).unwrap();

        assert!(jsonb.has_all_keys(&["a".to_string(), "b".to_string()]));
        assert!(jsonb.has_any_key(&["a".to_string(), "d".to_string()]));
        assert!(!jsonb.has_all_keys(&["a".to_string(), "d".to_string()]));
    }

    #[test]
    fn test_jsonb_concat() {
        let jsonb1 = JsonbValue::from_str(r#"{"a": 1}"#).unwrap();
        let jsonb2 = JsonbValue::from_str(r#"{"b": 2}"#).unwrap();

        let result = jsonb1.concat(&jsonb2);
        assert!(result.has_key("a"));
        assert!(result.has_key("b"));
    }

    #[test]
    fn test_jsonb_delete() {
        let jsonb = JsonbValue::from_str(r#"{"a": 1, "b": 2}"#).unwrap();
        let result = jsonb.delete_key("a");

        assert!(!result.has_key("a"));
        assert!(result.has_key("b"));
    }

    #[test]
    fn test_jsonb_build_object() {
        let pairs = vec![
            ("name".to_string(), Value::Text("Alice".to_string())),
            ("age".to_string(), Value::Integer(30)),
        ];

        let jsonb = JsonbFunctions::build_object(pairs).unwrap();
        assert!(jsonb.has_key("name"));
        assert!(jsonb.has_key("age"));
    }

    #[test]
    fn test_jsonb_build_array() {
        let values = vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)];

        let jsonb = JsonbFunctions::build_array(values).unwrap();
        assert_eq!(jsonb.to_string(), "[1,2,3]");
    }

    #[test]
    fn test_array_contains() {
        let array = vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)];
        assert!(ArrayOps::contains(&array, &Value::Integer(2)));
        assert!(!ArrayOps::contains(&array, &Value::Integer(5)));
    }

    #[test]
    fn test_array_overlaps() {
        let a = vec![Value::Integer(1), Value::Integer(2)];
        let b = vec![Value::Integer(2), Value::Integer(3)];
        let c = vec![Value::Integer(4), Value::Integer(5)];

        assert!(ArrayOps::overlaps(&a, &b)); // Share element 2
        assert!(!ArrayOps::overlaps(&a, &c)); // No common elements
    }

    #[test]
    fn test_array_concat() {
        let a = vec![Value::Integer(1), Value::Integer(2)];
        let b = vec![Value::Integer(3), Value::Integer(4)];

        let result = ArrayOps::concat(a, b);
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], Value::Integer(1));
        assert_eq!(result[3], Value::Integer(4));
    }

    #[test]
    fn test_array_unnest() {
        let array = vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)];

        let result = ArrayOps::unnest(array);
        assert_eq!(result.len(), 3);
    }
}
