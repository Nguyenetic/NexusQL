#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use nexusql::{NexusSQL, ResultSet as NexusResultSet, Row as NexusRow, Value as NexusValue};
use std::sync::Arc;
use tokio::sync::Mutex;

/// NexusDB - Main database client for Node.js
#[napi]
pub struct NexusDB {
    db: Arc<Mutex<NexusSQL>>,
    runtime: Arc<tokio::runtime::Runtime>,
}

#[napi]
impl NexusDB {
    #[napi(constructor)]
    pub fn new(path: String) -> Result<Self> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| Error::from_reason(format!("Failed to create runtime: {}", e)))?;

        let db = runtime
            .block_on(async { NexusSQL::new(&path).await })
            .map_err(|e| Error::from_reason(format!("Failed to create database: {}", e)))?;

        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            runtime: Arc::new(runtime),
        })
    }

    #[napi]
    pub async fn query(&self, sql: String) -> Result<QueryResult> {
        let db = self.db.clone();

        let result = db
            .lock()
            .await
            .execute(&sql)
            .await
            .map_err(|e| Error::from_reason(format!("Query failed: {}", e)))?;

        Ok(convert_result(result))
    }

    #[napi]
    pub async fn execute(&self, sql: String) -> Result<u32> {
        let result = self.query(sql).await?;
        Ok(result.row_count)
    }

    #[napi]
    pub async fn begin_transaction(&self) -> Result<Transaction> {
        let db = self.db.clone();

        // Execute BEGIN statement
        db.lock()
            .await
            .execute("BEGIN")
            .await
            .map_err(|e| Error::from_reason(format!("Transaction begin failed: {}", e)))?;

        Ok(Transaction { db, active: true })
    }

    #[napi]
    pub async fn close(&self) -> Result<()> {
        // Flush any pending operations
        let db = self.db.clone();
        db.lock()
            .await
            .execute("COMMIT")
            .await
            .map_err(|e| Error::from_reason(format!("Failed to flush on close: {}", e)))?;

        Ok(())
    }
}

#[napi]
pub struct Transaction {
    db: Arc<Mutex<NexusSQL>>,
    active: bool,
}

#[napi]
impl Transaction {
    #[napi]
    pub async fn query(&self, sql: String) -> Result<QueryResult> {
        if !self.active {
            return Err(Error::from_reason("Transaction already completed"));
        }

        let db = self.db.clone();
        let result = db
            .lock()
            .await
            .execute(&sql)
            .await
            .map_err(|e| Error::from_reason(format!("Query failed: {}", e)))?;

        Ok(convert_result(result))
    }

    #[napi]
    pub async fn commit(&mut self) -> Result<()> {
        if !self.active {
            return Err(Error::from_reason("Transaction already completed"));
        }

        let db = self.db.clone();
        db.lock()
            .await
            .execute("COMMIT")
            .await
            .map_err(|e| Error::from_reason(format!("Commit failed: {}", e)))?;

        self.active = false;
        Ok(())
    }

    #[napi]
    pub async fn rollback(&mut self) -> Result<()> {
        if !self.active {
            return Err(Error::from_reason("Transaction already completed"));
        }

        let db = self.db.clone();
        db.lock()
            .await
            .execute("ROLLBACK")
            .await
            .map_err(|e| Error::from_reason(format!("Rollback failed: {}", e)))?;

        self.active = false;
        Ok(())
    }
}

/// Query result structure
#[napi(object)]
#[derive(Clone)]
pub struct QueryResult {
    pub rows: Vec<serde_json::Value>,
    pub row_count: u32,
    pub fields: Vec<Field>,
}

/// Field metadata
#[napi(object)]
#[derive(Clone)]
pub struct Field {
    pub name: String,
    pub field_type: String,
}

// Helper functions

/// Convert NexusQL ResultSet to JavaScript QueryResult
fn convert_result(result: NexusResultSet) -> QueryResult {
    // Extract field names from first row if available
    let fields: Vec<Field> = if !result.rows.is_empty() {
        result.rows[0]
            .columns
            .iter()
            .map(|name| Field {
                name: name.clone(),
                field_type: "TEXT".to_string(),
            })
            .collect()
    } else {
        vec![]
    };

    QueryResult {
        rows: result.rows.iter().map(|row| convert_row(row)).collect(),
        row_count: result.rows_affected as u32,
        fields,
    }
}

/// Convert NexusQL Row to JSON
fn convert_row(row: &NexusRow) -> serde_json::Value {
    let mut obj = serde_json::Map::new();

    // Iterate over columns and values together
    for (i, column_name) in row.columns.iter().enumerate() {
        if let Some(value) = row.values.get(i) {
            let json_value = match value {
                NexusValue::Null => serde_json::Value::Null,
                NexusValue::Boolean(b) => serde_json::Value::Bool(*b),
                NexusValue::Integer(i) => serde_json::Value::Number((*i).into()),
                NexusValue::Float(f) => {
                    // OrderedFloat<f64> needs to be dereferenced twice
                    let float_val = **f;
                    serde_json::Number::from_f64(float_val)
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::Null)
                }
                NexusValue::Text(s) => serde_json::Value::String(s.clone()),
                NexusValue::Blob(b) => {
                    // Convert binary data to base64
                    serde_json::Value::String(base64_encode(b))
                }
            };

            obj.insert(column_name.clone(), json_value);
        }
    }

    serde_json::Value::Object(obj)
}

/// Simple base64 encoding
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();

    for chunk in data.chunks(3) {
        let b1 = chunk[0];
        let b2 = chunk.get(1).copied().unwrap_or(0);
        let b3 = chunk.get(2).copied().unwrap_or(0);

        result.push(CHARS[(b1 >> 2) as usize] as char);
        result.push(CHARS[(((b1 & 0x03) << 4) | (b2 >> 4)) as usize] as char);
        result.push(if chunk.len() > 1 {
            CHARS[(((b2 & 0x0F) << 2) | (b3 >> 6)) as usize] as char
        } else {
            '='
        });
        result.push(if chunk.len() > 2 {
            CHARS[(b3 & 0x3F) as usize] as char
        } else {
            '='
        });
    }

    result
}
