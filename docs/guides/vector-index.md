# HNSW vector index

`CREATE INDEX ... USING hnsw` builds an approximate nearest-neighbor index on a
`VECTOR(n)` column. It answers `ORDER BY <distance> LIMIT k` queries in well
under a millisecond where a scan computes every distance. AMen (shared agent
memory) uses it for meaning-based search over message embeddings.

## Quick start

```sql
CREATE TABLE docs (id INT, project TEXT, embedding VECTOR(768));
CREATE INDEX docs_hnsw ON docs USING hnsw (embedding) WITH (metric = 'cosine');

SELECT id, embedding <=> $1 AS distance
FROM docs
WHERE project = 'nexus'
ORDER BY distance
LIMIT 10;
```

From Python:

```python
db.create_vector_index("docs", "embedding", metric="cosine")
rows = db.vector_search("docs", query_vector, select=["id"], where="project = ?",
                        params=["nexus"], limit=10)   # adds a `distance` column
```

## Options

| Option | Default | Meaning |
|-|-|-|
| `metric` | `'l2'` (SQL), `'cosine'` (Python helper) | `'l2'` serves `<->`, `l2_distance()`, `vector_distance()`; `'cosine'` serves `<=>` and `cosine_distance()`. A query with the other metric scans. |
| `m` | 16 | Links per node (2-100). Higher raises recall and memory. |
| `ef_construction` | 64 | Candidate list while inserting. Higher builds a better graph, slower. |
| `ef_search` | 40 | Minimum candidate list while searching (raised to the number of rows requested). |

Unknown options, a metric other than `l2`/`cosine`, and values outside
1-10000 are rejected. Re-creating an index with the same name replaces it.

## When the index is used

A single-table `SELECT` with `ORDER BY <distance> [ASC] LIMIT k`, where the
distance is `col <-> q`, `col <=> q`, `l2_distance(col, q)`,
`vector_distance(col, q)` or `cosine_distance(col, q)` in either operand order,
`col` has an HNSW index of the same metric, and `q` is a literal or parameter.
A select-list alias (`AS distance ... ORDER BY distance`) is resolved first.
Joins, `GROUP BY`, aggregates, window functions and subqueries in `WHERE` use
the ordinary scan.

The graph proposes candidates; each is checked against the whole `WHERE`
clause and ordered by its exact distance. When a selective filter rejects too
many candidates the search widens (4x per round), and if the graph still cannot
supply `k` matching rows the query scans. Rows whose value cannot be indexed
(NULL, another dimension, non-finite, or a zero vector under cosine) are not in
the graph, so while any such row exists the query scans: results never omit
them.

Like every HNSW index the answer is approximate: a true neighbor the graph does
not reach is not returned. With the defaults, recall@10 is above 0.9 on the
test corpora (`tests/vector_index_test.rs`).

## Index behavior

- **Definition persisted, graph derived.** The definition is stored in the
  table schema and committed through the WAL. The graph is not written to disk;
  it is built from the rows by the first nearest-neighbor query after open or
  `CREATE INDEX`, so opening a database and keyword-only work never pay for it.
- **Maintenance.** INSERT, bulk insert, UPDATE and DELETE update a built graph
  in place. Deleted nodes are unlinked without repair; after more than a quarter
  of the graph has been deleted it is dropped and rebuilt on the next query.
- **Transactions.** `BEGIN` snapshots share the graph; the first write copies
  it, so `ROLLBACK` restores the graph that matches the restored rows.
- **Construction.** Neighbors are chosen with the HNSW paper's heuristic (keep
  a candidate only if it is closer to the node than to the links already kept,
  then fill with the nearest skipped ones), which keeps outliers reachable.

## Performance

`cargo test --release --test vector_index_test scale -- --ignored --nocapture`
(Windows x86-64, 20,000 clustered 768-dimension vectors, cosine):

| Operation | Time |
|-|-|
| First query after open (builds the graph) | 10.4 s |
| One indexed `ORDER BY e <=> $1 LIMIT 10` | 0.22 ms |
| One exact scan of the same table | 65 ms |

Building costs about 0.5 ms per 768-dimension vector. Long-lived processes
should issue one nearest-neighbor query at startup to build the graph before
users need it.
