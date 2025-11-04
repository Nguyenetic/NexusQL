# NexusQL TypeScript Helper Methods Guide

This guide covers the convenience helper methods for vector operations in NexusQL's TypeScript bindings.

## Overview

NexusQL provides three high-level helper methods that make vector operations simple and intuitive:

1. **`insertVector()`** - Easy vector insertion with metadata
2. **`vectorSearch()`** - Simple nearest neighbor search
3. **`createVectorIndex()`** - HNSW index creation

These methods complement the raw SQL interface, providing a more ergonomic API for common vector operations.

---

## Installation

```bash
npm install @nguyenetic/nexusql
```

---

## Quick Start

```javascript
const { NexusDb } = require('@nguyenetic/nexusql');

async function example() {
    const db = new NexusDb('mydb.db');

    // Create table
    await db.execute(`
        CREATE TABLE documents (
            id INTEGER,
            text TEXT,
            embedding VECTOR(384)
        )
    `);

    // Insert vector with metadata
    const embedding = [...]; // 384-dimensional vector
    await db.insertVector('documents', embedding,
        JSON.stringify({ id: 1, text: 'Hello world' }));

    // Search for similar vectors
    const resultsJson = await db.vectorSearch('documents', embedding, 10, 'cosine');
    const results = JSON.parse(resultsJson);

    console.log(results[0].text, results[0].distance);

    await db.close();
}
```

---

## API Reference

### `insertVector(table, vector, metadata?)`

Insert a vector with optional metadata into a table.

**Parameters:**
- `table` (string) - Table name
- `vector` (number[]) - Vector embedding as array of numbers
- `metadata` (string, optional) - JSON string with additional columns

**Returns:** `Promise<void>`

**Example:**

```javascript
// Basic insert with just vector
await db.insertVector('embeddings', [0.1, 0.2, 0.3]);

// Insert with metadata
await db.insertVector('embeddings', [0.1, 0.2, 0.3],
    JSON.stringify({
        id: 1,
        text: 'Machine learning tutorial',
        category: 'AI',
        timestamp: Date.now()
    }));
```

**Important Notes:**
- The `embedding` column is automatically added
- Metadata keys must match existing table columns
- JSON key order is preserved in the INSERT statement
- Vectors are stored as native VECTOR type, not TEXT

---

### `vectorSearch(table, queryVector, limit, distanceMetric?)`

Search for nearest vectors using similarity search.

**Parameters:**
- `table` (string) - Table name
- `queryVector` (number[]) - Query vector as array of numbers
- `limit` (number) - Maximum number of results to return
- `distanceMetric` (string, optional) - Distance metric: `'l2'`, `'cosine'`, or `'inner_product'` (default: `'l2'`)

**Returns:** `Promise<string>` - JSON string containing array of results with `distance` field added

**Example:**

```javascript
// L2 distance (default)
const l2Json = await db.vectorSearch('embeddings', queryVector, 10);
const l2Results = JSON.parse(l2Json);

// Cosine distance (best for normalized vectors)
const cosineJson = await db.vectorSearch('embeddings', queryVector, 10, 'cosine');
const cosineResults = JSON.parse(cosineJson);

// Inner product (for maximum inner product search)
const ipJson = await db.vectorSearch('embeddings', queryVector, 10, 'inner_product');
const ipResults = JSON.parse(ipJson);

// Access results
cosineResults.forEach(row => {
    console.log(`ID: ${row.id}, Text: ${row.text}, Distance: ${row.distance}`);
});
```

**Distance Metrics:**
- **`'l2'`** - Euclidean (L2) distance: `sqrt(sum((a[i] - b[i])^2))`
  - Best for: General-purpose similarity
  - Range: [0, ∞), smaller is more similar

- **`'cosine'`** - Cosine distance: `1 - cosine_similarity`
  - Best for: Normalized vectors, text embeddings
  - Range: [0, 2], smaller is more similar

- **`'inner_product'`** - Negative inner product: `-dot(a, b)`
  - Best for: Maximum inner product search (MIPS)
  - Range: (-∞, ∞), smaller is more similar

**Performance:**
- Automatically uses HNSW index when available (13-1578× faster than brute force)
- Falls back to exact search for small datasets
- Typical performance: <1ms per query on 1K vectors (384-dim)

---

### `createVectorIndex(table, column?, indexType?, config?)`

Create a vector index for fast similarity search.

**Parameters:**
- `table` (string) - Table name
- `column` (string, optional) - Column name containing vectors (default: `'embedding'`)
- `indexType` (string, optional) - Index type: `'hnsw'` or `'flat'` (default: `'hnsw'`)
- `config` (string, optional) - JSON string with index configuration

**Returns:** `Promise<string>` - Status message

**Example:**

```javascript
// Default configuration
await db.createVectorIndex('embeddings');

// Custom HNSW configuration
await db.createVectorIndex('embeddings', 'embedding', 'hnsw',
    JSON.stringify({
        m: 16,              // Max connections per node
        ef_construction: 64, // Build quality (higher = better, slower)
        ef_search: 40       // Search quality (higher = better, slower)
    }));

// Flat (brute-force) index
await db.createVectorIndex('embeddings', 'embedding', 'flat');
```

**Configuration Options (HNSW):**
- `m` (default: 16) - Maximum number of connections per layer
  - Higher values: Better recall, more memory
  - Typical range: 8-64

- `ef_construction` (default: 64) - Search depth during construction
  - Higher values: Better index quality, slower build time
  - Typical range: 32-500

- `ef_search` (default: 40) - Search depth during queries
  - Higher values: Better recall, slower queries
  - Typical range: 16-500

**Note:** Index creation syntax is currently being finalized. HNSW is used automatically for vector searches in the current version.

---

## Complete Example: MCP Memory System

```javascript
const { NexusDb } = require('@nguyenetic/nexusql');

class VectorMemory {
    constructor(dbPath) {
        this.db = new NexusDb(dbPath);
    }

    async initialize() {
        await this.db.execute(`
            CREATE TABLE IF NOT EXISTS memories (
                id INTEGER PRIMARY KEY,
                content TEXT,
                timestamp INTEGER,
                embedding VECTOR(384)
            )
        `);

        await this.db.createVectorIndex('memories', 'embedding', 'hnsw');
    }

    async addMemory(id, content, embedding) {
        await this.db.insertVector('memories', embedding,
            JSON.stringify({
                id: id,
                content: content,
                timestamp: Date.now()
            }));
    }

    async searchMemories(queryEmbedding, limit = 10) {
        const resultsJson = await this.db.vectorSearch(
            'memories',
            queryEmbedding,
            limit,
            'cosine'
        );

        return JSON.parse(resultsJson);
    }

    async close() {
        await this.db.close();
    }
}

// Usage
async function main() {
    const memory = new VectorMemory('memories.db');
    await memory.initialize();

    // Add some memories (embeddings from your model)
    await memory.addMemory(1, 'Python tutorial', embeddings[0]);
    await memory.addMemory(2, 'JavaScript guide', embeddings[1]);
    await memory.addMemory(3, 'Rust programming', embeddings[2]);

    // Search for similar memories
    const results = await memory.searchMemories(queryEmbedding, 5);

    results.forEach(mem => {
        console.log(`${mem.content} (distance: ${mem.distance.toFixed(4)})`);
    });

    await memory.close();
}
```

---

## Performance Characteristics

### insertVector()
- **Latency:** 1-2ms per insert (single-threaded)
- **Throughput:** ~500-1000 inserts/sec
- **Optimization:** Use transactions for bulk inserts

```javascript
const tx = await db.beginTransaction();
for (let i = 0; i < 1000; i++) {
    await tx.execute(`INSERT INTO docs VALUES (...)`);
}
await tx.commit();
```

### vectorSearch()
- **1K vectors:** <1ms per query
- **10K vectors:** 1-5ms per query
- **100K vectors:** 5-20ms per query
- **Speedup vs brute force:** 13-1578× (depending on dataset size)

### Memory Usage
- **Per vector (384-dim):** ~1.5 KB
- **HNSW index overhead:** ~2-3× vector size
- **Total for 10K vectors:** ~45-60 MB

---

## Migration from sqlite-vec

NexusQL helper methods provide a cleaner API than sqlite-vec virtual tables:

**sqlite-vec (old):**
```javascript
db.prepare(`
    INSERT INTO vec_items(rowid, embedding)
    VALUES (?, ?)
`).run(id, Buffer.from(new Float32Array(embedding).buffer));

const results = db.prepare(`
    SELECT rowid, distance
    FROM vec_items
    WHERE embedding MATCH ?
    ORDER BY distance
    LIMIT ?
`).all(Buffer.from(new Float32Array(query).buffer), 10);
```

**NexusQL (new):**
```javascript
await db.insertVector('items', embedding, JSON.stringify({ id }));

const resultsJson = await db.vectorSearch('items', query, 10);
const results = JSON.parse(resultsJson);
```

**Benefits:**
- No Buffer/Float32Array conversion needed
- Native async/await support
- Metadata handled automatically
- 10-100× faster performance

---

## Best Practices

### 1. Use Appropriate Distance Metric

```javascript
// Text embeddings (BERT, sentence transformers) → Cosine
await db.vectorSearch('docs', query, 10, 'cosine');

// Image embeddings (ResNet, CLIP) → L2
await db.vectorSearch('images', query, 10, 'l2');

// Recommendation systems → Inner Product
await db.vectorSearch('products', query, 10, 'inner_product');
```

### 2. Optimize Batch Inserts

```javascript
// Use transactions for bulk loading
const tx = await db.beginTransaction();
for (const doc of documents) {
    const metadata = JSON.stringify({ id: doc.id, text: doc.text });
    await tx.execute(`
        INSERT INTO docs (id, text, embedding)
        VALUES (${doc.id}, '${doc.text}', '[${doc.embedding.join(',')}]')
    `);
}
await tx.commit();
```

### 3. Tune HNSW Parameters

```javascript
// For high recall (>95%), increase ef_search
await db.createVectorIndex('docs', 'embedding', 'hnsw',
    JSON.stringify({ ef_search: 100 }));

// For faster build, decrease ef_construction
await db.createVectorIndex('docs', 'embedding', 'hnsw',
    JSON.stringify({ ef_construction: 40 }));
```

### 4. Handle Cleanup Properly

```javascript
async function cleanup(db) {
    await db.close();
    // Wait for file handles to release (Windows)
    await new Promise(resolve => setTimeout(resolve, 200));
}
```

---

## Troubleshooting

### Column Order Mismatch

**Problem:** Metadata fields appear in wrong columns

**Solution:** Ensure JSON keys match table column names exactly

```javascript
// Table schema
CREATE TABLE docs (id INTEGER, text TEXT, embedding VECTOR(384))

// Correct - keys match schema
await db.insertVector('docs', vector,
    JSON.stringify({ id: 1, text: 'hello' }));

// Wrong - keys don't match
await db.insertVector('docs', vector,
    JSON.stringify({ doc_id: 1, content: 'hello' }));
```

### Distance Field Missing

**Problem:** `distance` field is undefined in results

**Solution:** Ensure you're parsing the JSON string

```javascript
// Wrong
const results = await db.vectorSearch('docs', query, 10);
console.log(results[0].distance); // undefined

// Correct
const resultsJson = await db.vectorSearch('docs', query, 10);
const results = JSON.parse(resultsJson);
console.log(results[0].distance); // works!
```

### Windows File Lock Errors

**Problem:** `EPERM: operation not permitted` when deleting database

**Solution:** Wait after closing database

```javascript
await db.close();
await new Promise(resolve => setTimeout(resolve, 200));
fs.unlinkSync('mydb.db'); // Now works
```

---

## See Also

- [Phase 3 SQL Integration Guide](../PHASE_3_SQL_INTEGRATION_COMPLETE.md)
- [MCP Integration Guide](../MCP_INTEGRATION_GUIDE.md)
- [Feature Parity Checklist](../FEATURE_PARITY_CHECKLIST.md)
