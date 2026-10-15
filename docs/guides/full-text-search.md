# Full-text search

NexusQL has keyword full-text search with BM25 ranking, phrase and prefix
queries, boolean operators, highlighted snippets, and an inverted index that is
maintained by every write and survives reopen. It was designed to replace
SQLite FTS5 in AMen (shared agent memory), whose needs set the scope:

| AMen need | NexusQL |
|-|-|
| Ranked keyword search | `bm25(col, query)` (FTS5 sign) or `fts_score(col, query)` |
| Phrase and prefix queries | `"exact phrase"`, `prefix*` |
| Excerpts with highlighted matches | `snippet()`, `highlight()` |
| Filter by project, provider, date in the same query | ordinary `WHERE` predicates next to `MATCH` (`=`, `IN`, `BETWEEN`, `>=`, ...) |
| Incremental inserts of ~15k+ messages | index maintained on INSERT, bulk insert and UPDATE |
| Delete and re-import a source | DELETE / UPDATE maintain the index; transactions roll it back |

## Quick start

```sql
CREATE TABLE messages (id SERIAL, project TEXT, provider TEXT, created TEXT, content TEXT);
CREATE INDEX messages_fts ON messages USING fulltext (content);

INSERT INTO messages (project, provider, created, content)
VALUES ('nexus', 'claude', '2026-10-01', 'WAL recovery replays committed frames');

SELECT id,
       bm25(content, 'wal recovery') AS score,
       snippet(content, 'wal recovery', '[', ']', '...', 12) AS excerpt
FROM messages
WHERE content MATCH 'wal recovery'
  AND project = 'nexus' AND created >= '2026-10-01'
ORDER BY score
LIMIT 20;
```

From Python, `NexusDB.search()` builds that query (see
[the Python binding README](../../bindings/python/README.md#full-text-search)):

```python
rows = db.search("messages", "wal recovery", select=["id", "project"],
                 where="project = ? AND created >= ?", params=["nexus", "2026-10-01"],
                 limit=20)
```

## SQL surface

| Syntax | Meaning |
|-|-|
| `CREATE INDEX name ON t USING fulltext (col)` | Build an inverted index on one TEXT column. `fts` and `fts5` are aliases. Re-creating an index with the same name replaces it. |
| `col MATCH 'query'` | True when the column's text matches the query. Also `col @@ 'query'` and `fts_match(col, 'query')`. |
| `bm25(col, 'query' [, k1, b])` | Okapi BM25, **negated like FTS5**: lower is better, so `ORDER BY bm25(...)` puts the best match first. 0 for rows that do not match. Defaults k1 = 1.2, b = 0.75. |
| `fts_score(col, 'query' [, k1, b])` | The same score, positive: use `ORDER BY ... DESC`. |
| `snippet(col, 'query' [, open, close, ellipsis, max_tokens])` | An excerpt of at most `max_tokens` tokens (default 16, maximum 64) around the best cluster of matches, with matches wrapped in `open`/`close` (default `[`/`]`) and `ellipsis` (default `...`) where text was cut. |
| `highlight(col, 'query' [, open, close])` | The whole text with matches marked. |

The query may be a literal or a parameter (`$1` or `?`). Queries are validated
when the statement starts; a syntax error fails the statement with
`invalid full-text query: ...`. NULL text never matches, and only TEXT values
are indexed.

### Query syntax

FTS5-compatible subset:

| Query | Matches |
|-|-|
| `wal recovery` | both terms (implicit AND) |
| `"wal recovery"` | the terms adjacent and in order; `""` inside quotes is a literal quote |
| `recov*` | any term starting with `recov`; also `"wal recov*"` / `"wal recov"*` |
| `a AND b`, `a OR b`, `a NOT b` | boolean operators, **uppercase only** (as in FTS5; lowercase `and` is an ordinary word) |
| `NOT a` | every indexed text without `a` (FTS5 rejects this; NexusQL allows it) |
| `(a OR b) c` | grouping |

Precedence is `NOT` > `AND` > `OR`, so `a b OR c NOT d` means
`(a AND b) OR (c AND NOT d)`. A bare word that the tokenizer splits, such as
`e-mail` or `東京タワー`, is searched as a phrase. A query without any searchable
term (only punctuation) matches nothing. Not supported: `NEAR`, column filters
(`col: term`), the `^` initial-token operator, and `+` phrase concatenation.

## Tokenizer

One tokenizer serves the index, `MATCH`, ranking and snippets, so the index is
exact rather than approximate.

- A word is a maximal run of Unicode letters, digits and combining marks;
  everything else (spaces, punctuation, `_`, emoji) separates words.
- Words are case-folded and compatibility-normalized (NFKD): `Café`, `CAFE` and
  `cafe` are the same term, fullwidth `ＡＢＣ１２` is `abc12`, ligatures split.
  Diacritics are removed. No stemming and no stop words, like FTS5's default
  `unicode61` tokenizer.
- Han, hiragana, katakana and Hangul are written without spaces, so a run of
  them is indexed as overlapping **bigrams** (`東京タワー` -> `東京 京タ タワ ワー`)
  plus the run's last character. A query is tokenized the same way and matched
  as a phrase, so any substring of two or more characters is found. A single
  CJK character in a query matches as a prefix (`港` finds `港区`). Kana keep
  their voicing marks (NFKC), so `ガ` and `カ` differ, and halfwidth or
  decomposed kana match their fullwidth forms.

FTS5's `unicode61` treats an unspaced Japanese sentence as one token, so the
same query can find more Japanese matches in NexusQL than in FTS5.

## Ranking

`bm25` uses Okapi BM25 with the non-negative idf
`ln(1 + (N - df + 0.5) / (df + 0.5))`. Each phrase, prefix or term outside a
`NOT` contributes; a phrase counts its occurrences as term frequency, and a
prefix counts every matching term. `N`, the average length and `df` are the
corpus statistics of the queried column in the statement's main table, taken
from the full-text index (or computed by scanning the column when it has no
index). Scores of the same query are comparable within a table; they are not
numerically identical to FTS5's, which uses a different idf.

Statistics are only available when the query is a literal or parameter and the
column belongs to the `FROM` table. Otherwise (a query computed per row, or a
column of a joined table) BM25 falls back to treating the row as a one-document
corpus, which still matches but ranks less meaningfully.

## Index behavior

- **Use.** `WHERE` clauses take index candidates from `col MATCH q` and
  `fts_match(col, q)` when `col` has a full-text index and `q` is a literal or
  parameter. Under `AND`, candidates from several indexed predicates (full-text
  or spatial) intersect; under `OR` every branch must be indexable, otherwise
  the table is scanned. The complete `WHERE` clause is still evaluated on the
  candidates, so results never depend on whether an index exists.
- **Maintenance.** INSERT (literal, parameterized and `bulk_insert`), UPDATE and
  DELETE update the index in place. Deleting rows does not rewrite posting
  lists; internal document ids are compacted once deleted entries outnumber
  live ones.
- **Transactions.** `BEGIN` snapshots tables together with their indexes, so
  `ROLLBACK` and failed statements restore the index exactly.
- **Durability.** The index is derived data and is never written to disk. Its
  definition is stored in the table schema, which is committed through the WAL
  with the rows (see [disk durability](../development/durability.md)); on open
  the postings are rebuilt from the recovered rows, as GiST indexes are. A
  killed process therefore recovers exactly the committed text, and the index
  cannot disagree with the table. Schemas written before this feature still
  load.
- **Memory and open time.** Postings live in memory, about 50 bytes per
  distinct word per row (an estimate from the data layout, several times the
  size of short texts). Building the index for 20,000 messages takes about
  0.12 s in a release build, paid at `CREATE INDEX` and at every open.

There is no `DROP INDEX` statement in NexusQL yet; dropping the table removes
its index. One index covers one column: index `title` and `body` separately and
combine `title MATCH q OR body MATCH q`.

## Performance

`cargo bench --bench fulltext_search` (release, Windows x86-64, in-memory
executor, 20,000 messages of 6-24 words, 8 projects). Each query is
`SELECT id, bm25(..), snippet(..) WHERE <match> AND project = 'p3' ORDER BY bm25 LIMIT 10`;
the `top5` rows drop the project filter and use `LIMIT 5`, the shape of a memory
search for a common word. The SQLite column is the same query against an FTS5
table with bundled SQLite. "Before" is the release that re-tokenized every
candidate row (2026-10-15, same corpus).

| Query | Matching rows (of 20,000) | FULLTEXT index | Before | Table scan | SQLite FTS5 |
|-|-|-|-|-|-|
| rare term `zebracorn` | 19 | 22 µs | 23 µs | 53 ms | 19 µs |
| common term `kaka` | 87 | 96 µs | 141 µs | 51 ms | 72 µs |
| prefix `kalo*` | 487 | 0.34 ms | 0.67 ms | 51 ms | 0.37 ms |
| boolean `checkpoint AND NOT recovery OR 検索` | 1,000 | 0.56 ms | 0.50 ms | 50 ms | 0.11 ms |
| phrase `"wal recovery"` | 2,715 | 1.6 ms | 4.9 ms | 54 ms | 1.7 ms |
| frequent `access` | 6,334 | 2.0 ms | 9.3 ms | 55 ms | 3.5 ms |
| top5 `access` | 6,334 | 1.6 ms | 39.6 ms | | 2.2 ms |
| top5 `access OR kaka` | 6,400 | 1.8 ms | 36.6 ms | | 2.3 ms |

| Operation | Time |
|-|-|
| `CREATE INDEX` over 20,000 rows | 114 ms |
| one parameterized INSERT into the indexed table | 3.0 µs |
| 4,000 parameterized INSERTs in one transaction, disk, FULL sync | 42 ms |

How a ranked search avoids touching rows: candidates come from the posting
lists; for `ORDER BY bm25(col, q) [LIMIT k]` (or `fts_score ... DESC`) every
candidate's score is computed from postings (term frequencies, positions for
phrases, document lengths), the candidates are sorted, and the rest of `WHERE`
is evaluated in rank order until `k` rows pass. Only those rows are cloned, and
only the returned rows are tokenized, for `snippet()` / `highlight()`. Other
shapes (no `LIMIT`, `ORDER BY` another column, `COUNT(*)`) still evaluate
every candidate, but `MATCH` and `bm25()` look the row's text up in the
statement's postings instead of tokenizing it. The shortcut is skipped (the
general path runs) for joins, aliases, `GROUP BY`, aggregates, window
functions, subqueries in `WHERE`, and when a candidate's ranked column is NULL.

Writes: each COMMIT (and each autocommit statement) writes every row of the
tables it changed to the WAL, a property of the current storage engine. Import
batches inside one transaction or with `bulk_insert`; committing 15,000
single-row autocommits rewrites the growing table 15,000 times.

## Migrating from SQLite FTS5

| SQLite FTS5 | NexusQL |
|-|-|
| `CREATE VIRTUAL TABLE m USING fts5(content, project UNINDEXED)` | `CREATE TABLE m (id SERIAL, project TEXT, content TEXT)` + `CREATE INDEX m_fts ON m USING fulltext (content)` |
| `WHERE m MATCH 'q'` | `WHERE content MATCH 'q'` |
| `ORDER BY bm25(m)` / `ORDER BY rank` | `ORDER BY bm25(content, 'q')` |
| `snippet(m, 0, '[', ']', '...', 10)` | `snippet(content, 'q', '[', ']', '...', 10)` |
| `highlight(m, 0, '[', ']')` | `highlight(content, 'q', '[', ']')` |
| `rowid` | your own key column (`SERIAL` assigns ids) |
| external-content tables and sync triggers | not needed: the index follows the table |

Pass the query once per function (or bind the same `?` value several times).

## Design decisions

- **Index the definition, rebuild the postings.** Persisting postings would add
  a second on-disk structure to keep consistent with the WAL; rebuilding from
  recovered rows reuses the existing guarantees and makes index corruption
  impossible, at the cost of open-time work (about 6 µs per message).
- **Exact candidates, re-checked from postings.** The index returns exactly the
  rows the per-row predicate accepts, and the executor still evaluates the full
  `WHERE` clause. The re-check and `bm25()` no longer re-tokenize the row: the
  statement keeps, per query, each matching text with its token count and
  phrase occurrence counts read from postings, and a text found there is a
  match with those counts (both depend only on the text, so this holds for any
  row or column carrying it). A text not found is tokenized as before. Both
  paths feed one BM25 function, so indexed and unindexed tables return
  identical rows, order and bit-identical scores, verified by
  `postings_ranking_matches_tokenized_rows` in `tests/fulltext_search_test.rs`
  (common words, phrases, prefixes, AND/OR/NOT, Japanese, filters, ties, DML).
  Prefix postings keep duplicate positions: a CJK bigram and the trailing
  unigram share a position and count as two occurrences, as when tokenizing.
- **FTS5 sign for `bm25`.** Lower-is-better keeps `ORDER BY bm25(...)` working
  unchanged for code ported from FTS5; `fts_score` serves `DESC` ordering.
- **Bigrams for CJK.** Dictionary segmentation would need large data files; bigrams
  give substring search with no dictionary, the approach of Lucene's CJK
  analyzer.
- **No stemming or stop words**, matching FTS5's default tokenizer, so `running`
  does not match `run`; use `run*`.
