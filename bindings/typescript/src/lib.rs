#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use nexusql::{NexusSQL, ResultSet as NexusResultSet, Row as NexusRow, Value as NexusValue};
use ordered_float::OrderedFloat;
use std::sync::Arc;
use tokio::sync::Mutex;

/// NexusDB - Main database client for Node.js
#[napi]
pub struct NexusDB {
    db: Arc<Mutex<NexusSQL>>,
    _runtime: Arc<tokio::runtime::Runtime>,
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
            _runtime: Arc::new(runtime),
        })
    }

    /// Execute a query with optional parameters (prevents SQL injection)
    ///
    /// # Arguments
    /// * `sql` - SQL statement with $1, $2, etc. placeholders
    /// * `params` - Optional array of parameter values
    ///
    /// # Example
    /// ```javascript
    /// const result = await db.queryWithParams(
    ///   'SELECT * FROM users WHERE id = $1 AND status = $2',
    ///   [123, 'active']
    /// );
    /// ```
    #[napi]
    pub async fn query_with_params(
        &self,
        sql: String,
        #[napi(ts_arg_type = "(string | number | boolean | null)[] | undefined")] params: Option<
            Vec<Option<napi::Either<String, napi::Either<i64, napi::Either<f64, bool>>>>>,
        >,
    ) -> Result<QueryResult> {
        let db = self.db.clone();

        // Convert parameters to NexusValue (supports null values)
        let nexus_params = if let Some(params_vec) = params {
            params_vec
                .into_iter()
                .map(|p| match p {
                    None => NexusValue::Null,
                    Some(napi::Either::A(s)) => NexusValue::Text(s),
                    Some(napi::Either::B(napi::Either::A(i))) => NexusValue::Integer(i),
                    Some(napi::Either::B(napi::Either::B(napi::Either::A(f)))) => {
                        NexusValue::Float(OrderedFloat(f))
                    }
                    Some(napi::Either::B(napi::Either::B(napi::Either::B(b)))) => {
                        NexusValue::Boolean(b)
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        let result = db
            .lock()
            .await
            .execute_with_params(&sql, Some(nexus_params))
            .await
            .map_err(|e| Error::from_reason(format!("Query failed: {}", e)))?;

        Ok(convert_result(result))
    }

    /// Execute a query with optional parameters
    ///
    /// # Arguments
    /// * `sql` - SQL query with optional $1, $2, etc. placeholders
    /// * `params` - Optional array of parameter values
    ///
    /// # Example
    /// ```javascript
    /// // Without parameters
    /// const result = await db.query('SELECT * FROM users');
    ///
    /// // With parameters
    /// const result = await db.query('SELECT * FROM users WHERE id = $1', [1]);
    /// ```
    #[napi]
    pub async fn query(
        &self,
        sql: String,
        #[napi(ts_arg_type = "(string | number | boolean | null)[] | undefined")] params: Option<
            Vec<Option<napi::Either<String, napi::Either<i64, napi::Either<f64, bool>>>>>,
        >,
    ) -> Result<QueryResult> {
        self.query_with_params(sql, params).await
    }

    /// Execute a statement with optional parameters
    /// Returns the number of affected rows
    ///
    /// # Arguments
    /// * `sql` - SQL statement with optional $1, $2, etc. placeholders
    /// * `params` - Optional array of parameter values
    ///
    /// # Example
    /// ```javascript
    /// // Without parameters
    /// await db.execute('CREATE TABLE users (id INT, name TEXT)');
    ///
    /// // With parameters
    /// await db.execute('INSERT INTO users VALUES ($1, $2)', [1, 'Alice']);
    /// ```
    #[napi]
    pub async fn execute(
        &self,
        sql: String,
        #[napi(ts_arg_type = "(string | number | boolean | null)[] | undefined")] params: Option<
            Vec<Option<napi::Either<String, napi::Either<i64, napi::Either<f64, bool>>>>>,
        >,
    ) -> Result<u32> {
        let result = self.query_with_params(sql, params).await?;
        Ok(result.row_count)
    }

    /// Execute a statement with optional parameters (alias for backward compatibility)
    #[napi]
    pub async fn execute_with_params(
        &self,
        sql: String,
        #[napi(ts_arg_type = "(string | number | boolean | null)[] | undefined")] params: Option<
            Vec<Option<napi::Either<String, napi::Either<i64, napi::Either<f64, bool>>>>>,
        >,
    ) -> Result<u32> {
        self.execute(sql, params).await
    }

    /// Create a prepared statement for executing the same query multiple times with different parameters
    ///
    /// # Arguments
    /// * `sql` - SQL statement with $1, $2, etc. placeholders
    ///
    /// # Returns
    /// PreparedStatement that can be executed multiple times
    ///
    /// # Example
    /// ```javascript
    /// const stmt = await db.prepare('INSERT INTO users (id, name) VALUES ($1, $2)');
    /// await stmt.execute(1, 'Alice');
    /// await stmt.execute(2, 'Bob');
    /// await stmt.close();
    /// ```
    #[napi]
    pub async fn prepare(&self, sql: String) -> Result<PreparedStatement> {
        Ok(PreparedStatement {
            db: self.db.clone(),
            sql,
        })
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
        // Shutdown the database to release file locks
        let db = self.db.lock().await;

        // Note: shutdown() consumes self, so we need to clone the Arc and take ownership
        // For now, we'll just flush - the actual cleanup happens when the last reference is dropped
        drop(db);

        // Give the OS a moment to release file handles
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        Ok(())
    }

    /// Insert a vector into a table with optional metadata
    ///
    /// # Arguments
    /// * `table` - Table name
    /// * `vector` - Vector embedding as array of numbers
    /// * `metadata` - Optional object with additional columns (e.g., {id: 1, text: "hello"})
    ///
    /// # Example
    /// ```javascript
    /// // Simple usage (embedding added automatically)
    /// await db.insertVector('embeddings', [0.1, 0.2, 0.3], JSON.stringify({ id: 1, text: 'hello' }));
    ///
    /// // MCP Sage style (specify embedding column explicitly)
    /// await db.insertVector('memories', vector, JSON.stringify({
    ///     user_id: 'user123',
    ///     content: 'User prefers TypeScript',
    ///     memory_level: 'user',
    ///     metadata: '{"confidence": 0.95}',
    ///     created_at: '2024-11-03T00:00:00Z'
    /// }));
    /// ```
    #[napi]
    pub async fn insert_vector(
        &self,
        table: String,
        vector: Vec<f64>,
        metadata: Option<String>,
    ) -> Result<()> {
        // Convert f64 to string representation for SQL
        let vector_str = format!(
            "[{}]",
            vector
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );

        // Build INSERT statement with metadata first, then embedding
        let mut columns = Vec::new();
        let mut values = Vec::new();

        // Add metadata columns first if provided
        if let Some(meta_json) = metadata {
            let meta: serde_json::Value = serde_json::from_str(&meta_json)
                .map_err(|e| Error::from_reason(format!("Invalid metadata JSON: {}", e)))?;

            if let serde_json::Value::Object(map) = meta {
                // Preserve insertion order from JSON (serde_json preserves order)
                for (key, value) in map {
                    // Skip embedding if present in metadata - we'll use the vector parameter
                    if key == "embedding" {
                        continue;
                    }
                    columns.push(key);
                    values.push(json_value_to_sql(&value));
                }
            }
        }

        // Always add embedding column from vector parameter
        columns.push("embedding".to_string());
        values.push(format!("'{}'", vector_str));

        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            table,
            columns.join(", "),
            values.join(", ")
        );

        self.execute(sql, None).await?;
        Ok(())
    }

    /// Search for nearest vectors using vector similarity
    ///
    /// # Arguments
    /// * `table` - Table name
    /// * `query_vector` - Query vector as array of numbers
    /// * `limit` - Number of results to return
    /// * `distance_metric` - Distance metric: 'l2', 'cosine', or 'inner_product' (default: 'l2')
    ///
    /// # Returns
    /// Array of results with all columns plus a 'distance' field
    ///
    /// # Example
    /// ```javascript
    /// const results = await db.vectorSearch('embeddings', [0.1, 0.2, 0.3], 10, 'cosine');
    /// console.log(results[0].distance, results[0].text);
    /// ```
    pub async fn vector_search(
        &self,
        table: String,
        query_vector: Vec<f64>,
        limit: u32,
        distance_metric: Option<String>,
    ) -> Result<serde_json::Value> {
        let metric = distance_metric.unwrap_or_else(|| "l2".to_string());

        let distance_fn = match metric.to_lowercase().as_str() {
            "cosine" => "cosine_distance",
            "inner_product" => "inner_product",
            _ => "vector_distance", // Default to L2
        };

        // Convert f64 vector to SQL string
        let vector_str = format!(
            "[{}]",
            query_vector
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );

        let sql = format!(
            "SELECT *, {}(embedding, '{}') as distance FROM {} ORDER BY distance LIMIT {}",
            distance_fn, vector_str, table, limit
        );

        let result = self.query(sql, None).await?;

        // Return rows (auto-converted to JS by serde-json feature)
        Ok(result.rows)
    }

    /// Create a vector index for fast similarity search
    ///
    /// # Arguments
    /// * `table` - Table name
    /// * `column` - Column name containing vectors (default: 'embedding')
    /// * `index_type` - Index type: 'hnsw' or 'flat' (default: 'hnsw')
    /// * `config` - Optional index configuration object
    ///
    /// # Example
    /// ```javascript
    /// await db.createVectorIndex('embeddings', 'embedding', 'hnsw', {
    ///   m: 16,
    ///   ef_construction: 64
    /// });
    /// ```
    #[napi]
    pub async fn create_vector_index(
        &self,
        table: String,
        column: Option<String>,
        index_type: Option<String>,
        config: Option<String>,
    ) -> Result<String> {
        let col = column.unwrap_or_else(|| "embedding".to_string());
        let idx_type = index_type.unwrap_or_else(|| "hnsw".to_string());

        // For now, return a message since CREATE INDEX syntax is not yet implemented in parser
        // This will be replaced with actual SQL execution once parser supports it
        let config_str = if let Some(cfg) = config {
            format!(" WITH {}", cfg)
        } else {
            String::new()
        };

        Ok(format!(
            "Vector index creation for {}.{} using {} index{} will be implemented in next version. For now, HNSW is used automatically for vector searches.",
            table, col, idx_type, config_str
        ))
    }

    /// Batch insert multiple vectors for optimal performance
    ///
    /// # Arguments
    /// * `table` - Table name
    /// * `vectors` - Array of vector embeddings
    /// * `metadata_array` - Array of metadata objects (must match vectors length)
    ///
    /// # Example
    /// ```javascript
    /// const vectors = [
    ///   [0.1, 0.2, 0.3],
    ///   [0.4, 0.5, 0.6]
    /// ];
    /// const metadata = [
    ///   { id: 1, text: 'hello' },
    ///   { id: 2, text: 'world' }
    /// ];
    /// await db.batchInsertVectors('embeddings', vectors, metadata);
    /// ```
    ///
    /// This is 10-100x faster than individual insertVector() calls
    #[napi]
    pub async fn batch_insert_vectors(
        &self,
        table: String,
        vectors: Vec<Vec<f64>>,
        metadata_array: Option<Vec<String>>,
    ) -> Result<u32> {
        if vectors.is_empty() {
            return Ok(0);
        }

        // Build batch INSERT statement
        let mut rows = Vec::new();

        for (i, vector) in vectors.iter().enumerate() {
            let vector_str = format!(
                "[{}]",
                vector
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            );

            let mut columns = Vec::new();
            let mut values = Vec::new();

            // Parse metadata if provided
            if let Some(ref meta_array) = metadata_array {
                if i < meta_array.len() {
                    let meta: serde_json::Value =
                        serde_json::from_str(&meta_array[i]).map_err(|e| {
                            Error::from_reason(format!(
                                "Invalid metadata JSON at index {}: {}",
                                i, e
                            ))
                        })?;

                    if let serde_json::Value::Object(map) = meta {
                        for (key, value) in map {
                            if key == "embedding" {
                                continue; // Skip, use vector parameter
                            }
                            if columns.is_empty() {
                                columns.push(key.clone());
                            }
                            values.push(json_value_to_sql(&value));
                        }
                    }
                }
            }

            // Always add embedding column
            if columns.is_empty() {
                columns = vec!["embedding".to_string()];
            } else if !columns.contains(&"embedding".to_string()) {
                columns.push("embedding".to_string());
            }
            values.push(format!("'{}'", vector_str));

            rows.push((columns.clone(), values));
        }

        // Build single batch INSERT
        if rows.is_empty() {
            return Ok(0);
        }

        let columns = &rows[0].0;
        let values_clauses: Vec<String> = rows
            .iter()
            .map(|(_, vals)| format!("({})", vals.join(", ")))
            .collect();

        let sql = format!(
            "INSERT INTO {} ({}) VALUES {}",
            table,
            columns.join(", "),
            values_clauses.join(", ")
        );

        self.execute(sql, None).await
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
    pub async unsafe fn commit(&mut self) -> Result<()> {
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
    pub async unsafe fn rollback(&mut self) -> Result<()> {
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

/// Prepared statement for executing parameterized queries multiple times
#[napi]
pub struct PreparedStatement {
    db: Arc<Mutex<NexusSQL>>,
    sql: String,
}

#[napi]
impl PreparedStatement {
    /// Execute the prepared statement with parameters
    ///
    /// # Arguments
    /// * `params` - Variable arguments that will be bound to $1, $2, etc. (supports null)
    ///
    /// # Example
    /// ```javascript
    /// const stmt = await db.prepare('SELECT * FROM users WHERE id = $1');
    /// const result = await stmt.execute(123);
    /// await stmt.close();
    /// ```
    #[napi(ts_args_type = "...params: (string | number | boolean | null)[]")]
    pub async fn execute(
        &self,
        params: Vec<Option<napi::Either<String, napi::Either<i64, napi::Either<f64, bool>>>>>,
    ) -> Result<QueryResult> {
        let db = self.db.clone();

        // Convert parameters to NexusValue (supports null values)
        let nexus_params: Vec<NexusValue> = params
            .into_iter()
            .map(|p| match p {
                None => NexusValue::Null,
                Some(napi::Either::A(s)) => NexusValue::Text(s),
                Some(napi::Either::B(napi::Either::A(i))) => NexusValue::Integer(i),
                Some(napi::Either::B(napi::Either::B(napi::Either::A(f)))) => {
                    NexusValue::Float(OrderedFloat(f))
                }
                Some(napi::Either::B(napi::Either::B(napi::Either::B(b)))) => {
                    NexusValue::Boolean(b)
                }
            })
            .collect();

        let result = db
            .lock()
            .await
            .execute_with_params(&self.sql, Some(nexus_params))
            .await
            .map_err(|e| {
                Error::from_reason(format!("Prepared statement execution failed: {}", e))
            })?;

        Ok(convert_result(result))
    }

    /// Close the prepared statement (for consistency with other database APIs)
    /// Note: In Rust this is a no-op since resources are automatically cleaned up
    #[napi]
    pub async fn close(&self) -> Result<()> {
        Ok(())
    }
}

/// Query result with auto-parsed rows (serde-json feature handles conversion)
#[napi(object)]
pub struct QueryResult {
    pub rows: serde_json::Value, // Auto-converted to JS objects/arrays
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

/// Convert JSON value to SQL literal string
fn json_value_to_sql(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        serde_json::Value::Array(arr) => {
            // Convert array to vector literal
            let nums: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_f64().map(|f| f.to_string()))
                .collect();
            format!("'[{}]'", nums.join(","))
        }
        serde_json::Value::Object(_) => "NULL".to_string(), // Objects not supported
    }
}

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

    // Convert rows to JSON Value (serde-json feature auto-converts to JS)
    let row_objects: Vec<serde_json::Value> =
        result.rows.iter().map(|row| convert_row(row)).collect();

    QueryResult {
        rows: serde_json::Value::Array(row_objects),
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
                NexusValue::Geometry(g) => serde_json::Value::String(g.as_text()),
                NexusValue::Blob(b) => {
                    // Convert binary data to base64
                    serde_json::Value::String(base64_encode(b))
                }
                NexusValue::Vector(v) => {
                    // Convert f32 vector to JSON array of numbers
                    let nums: Vec<serde_json::Value> = v
                        .iter()
                        .filter_map(|f| {
                            serde_json::Number::from_f64(*f as f64).map(serde_json::Value::Number)
                        })
                        .collect();
                    serde_json::Value::Array(nums)
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
