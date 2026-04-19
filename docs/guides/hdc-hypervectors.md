# Hyperdimensional Computing (HDC) Hypervectors

NexusQL ships a first-class hypervector type with SQL-native operations.
Use it when you want **one column to carry many attributes of a record
and survive typos, missing fields, and operator variance** — the kind
of problems clean relational predicates don't handle well.

This page covers:

- [When HDC is the right tool](#when-hdc-is-the-right-tool)
- [Quick start](#quick-start)
- [The `hv_*` function surface](#the-hv_-function-surface)
- [Supported variants (BSC, MAP)](#supported-variants)
- [Real-world example — QA-hold similarity search](#real-world-example)
- [Performance](#performance)
- [On-disk format](#on-disk-format)
- [Roadmap](#roadmap)

---

## When HDC is the right tool

HDC is designed for data where *a record is more than the sum of its
columns*. You want to find records with the **same overall pattern**,
not records whose fields match one-for-one.

**Use HDC when:**
- Free-text fields carry information that operators phrase differently
  (comments, dispositions, descriptions, notes)
- Categorical fields have inconsistent spellings ("Die 5" vs "Die-5")
- Records are often missing fields and equality-joins would drop them
- You want "find things like this" without hand-tuning column weights
- Anomaly detection over multi-attribute records

**Don't use HDC when:**
- You need exact matches on a key (use a normal index)
- Numeric similarity on dense feature vectors (use `Vector` + HNSW)
- Your data has a clean schema and every field matters independently

---

## Quick start

Store a hypervector in any `BLOB` column. Build, query, and compare
with the `hv_*` scalar functions.

```sql
-- 1. Create a table with a hypervector column
CREATE TABLE documents (
    id INT,
    title TEXT,
    hv BLOB
);

-- 2. Encode "cat sat on mat" as a MAP hypervector
INSERT INTO documents (id, title, hv) VALUES (
    1, 'cat sat on mat',
    hv_bundle(
        hv_atom('tok', 'cat', 10000, 'MAP'),
        hv_atom('tok', 'sat', 10000, 'MAP'),
        hv_atom('tok', 'on',  10000, 'MAP'),
        hv_atom('tok', 'mat', 10000, 'MAP')
    )
);

INSERT INTO documents (id, title, hv) VALUES (
    2, 'dog on mat',
    hv_bundle(
        hv_atom('tok', 'dog', 10000, 'MAP'),
        hv_atom('tok', 'on',  10000, 'MAP'),
        hv_atom('tok', 'mat', 10000, 'MAP')
    )
);

-- 3. Find documents similar to a query
SELECT hv INTO @query FROM (
    VALUES (hv_bundle(
        hv_atom('tok', 'cat', 10000, 'MAP'),
        hv_atom('tok', 'mat', 10000, 'MAP')
    ))
);

SELECT id, title, hv_similarity(hv, $1) AS score
FROM documents
ORDER BY score DESC;
-- → 1 ("cat sat on mat")   score ~0.40
-- → 2 ("dog on mat")        score ~0.25
```

The "cat sat on mat" record scores highest because two of its tokens
(cat, mat) match the query. "dog on mat" shares only `mat`. Neither
contains the exact query "cat mat" — but HDC recognises the pattern
overlap.

---

## The `hv_*` function surface

| Function | Returns | Purpose |
|-|-|-|
| `hv_atom(kind, key, dim, variant)` | BLOB | Deterministic hypervector from a string key. Same (kind, key) → same vector. |
| `hv_random(dim, variant[, seed])` | BLOB | Random hypervector from an integer seed. |
| `hv_bind(a, b)` | BLOB | A ⊗ B — quasi-orthogonal to both inputs. |
| `hv_bundle(a1, a2, ...)` | BLOB | Superposition of N hypervectors (scalar form). |
| `hv_permute(a, shift)` | BLOB | Cyclic rotation — encodes sequence position. |
| `hv_similarity(a, b)` | REAL | Cosine similarity. [−1, +1] for MAP, [0, 1] for BSC. |
| `hv_variant(a)` | TEXT | "BSC", "MAP", etc. |
| `hv_dim(a)` | INTEGER | Conceptual dimension. |

**NULL handling.** Every `hv_*` function returns `NULL` on bad input
(wrong type, unknown variant, dimension mismatch, corrupt bytes, mixed
variants). No exceptions, no panics — matches the rest of NexusQL's
scalar function convention.

**Parameters.** Hypervectors pass cleanly through `$1`/`$2` parameters
since they're stored as `Value::Blob`. Fetch a seed into Python, pass
it back as a parameter to a similarity query.

---

## Supported variants

### BSC — Binary Spatter Codes

Vectors are packed bits, `H ∈ {0, 1}^D`. Binding is XOR, bundling is
majority vote, similarity is `1 − 2·d_H/D`.

**Use BSC when:** you want the fastest per-op throughput and don't
need a "negative correlation" signal from similarity.

**Similarity range:** [0, 1] — "unrelated" maps to 0.5, not 0.

### MAP — Multiply-Add-Permute

Vectors are bipolar ±1, `H ∈ {−1, +1}^D`, stored packed on disk.
Binding is Hadamard product, bundling is sign-of-sum, similarity is
plain cosine.

**Use MAP when:** you want signed similarity — "opposite" concepts
score near −1, "unrelated" near 0, "same" at +1. Default for new
records unless you have a reason to pick BSC.

**Similarity range:** [−1, +1].

**Why signed matters:** makes analogical reasoning ("dollar is to USA
as peso is to Mexico") and signed-class classifiers cleaner without
a rescale. Same per-op cost as BSC.

### Cross-variant operations return NULL

By design — every VSA has its own binding operator, so binding a BSC
vector with a MAP vector is mathematically undefined. The evaluator
returns `NULL` instead of raising so you can mix variants in a query
without tripping error handling:

```sql
-- Safe even if hv is sometimes BSC, sometimes MAP
SELECT id, COALESCE(hv_similarity(hv, $1), 0.0) AS sim
FROM records
ORDER BY sim DESC LIMIT 10;
```

---

## Real-world example

Production data from a manufacturing QA-hold system. 2,363 records,
each with:
- 10 structured fields (Status, Reason, Item_code, Die_no, operator
  IDs, …)
- 5 free-text fields (Hold_comment, Judgement_instruction, operator
  notes)
- Heavy typo / phrasing variance — different operators describe the
  same defect pattern in completely different words

### Encoding

```sql
CREATE TABLE hold_hvs (row_idx INT, hold_no INT, hv BLOB);

-- Per row — the bulk-load mode makes this a single disk write
-- regardless of how many INSERTs run:
INSERT INTO hold_hvs (row_idx, hold_no, hv) VALUES (
    14, 15,
    hv_bundle(
        -- Structured fields: one role⊗value bind per field
        hv_bind(hv_atom('role', 'Status',    10000, 'MAP'),
                hv_atom('val:Status', 'PROCESSED', 10000, 'MAP')),
        hv_bind(hv_atom('role', 'Reason',    10000, 'MAP'),
                hv_atom('val:Reason', 'PROMEX',    10000, 'MAP')),
        hv_bind(hv_atom('role', 'Item_code', 10000, 'MAP'),
                hv_atom('val:Item_code', 'L42P-FR6', 10000, 'MAP')),
        -- ... one per structured field

        -- Free text: bundle token atoms, bind result to the role
        hv_bind(hv_atom('role', 'Judgement_instruction', 10000, 'MAP'),
                hv_bundle(
                    hv_atom('tok', 're',      10000, 'MAP'),
                    hv_atom('tok', 'measure', 10000, 'MAP'),
                    hv_atom('tok', 'ng',      10000, 'MAP'),
                    hv_atom('tok', 'sample',  10000, 'MAP')
                    -- ... one per token (typical 20-50)
                ))
    )
);
```

### Query: "find holds like #15"

```sql
-- Fetch the seed's hypervector
SELECT hv INTO @seed FROM hold_hvs WHERE row_idx = 14;

-- Rank everything else by similarity
SELECT row_idx, hold_no, hv_similarity(hv, $1) AS sim
FROM hold_hvs
WHERE row_idx != 14
ORDER BY sim DESC
LIMIT 5;
```

Result: 5 holds matching the underlying pattern (PROMEX reason,
IMPORT-origin, overlapping terminology about re-measuring NG samples)
even though their exact field values and wording differed from the
seed.

### Numbers

| Stage | numpy baseline | NexusQL SQL |
|-|-|-|
| Encoding 2363 holds | 1.5 s | 2.4 s |
| Top-5 similarity over 2362 candidates | 0.01 s | 0.01 s |

SQL is 0.9 s slower on encoding due to PyO3 per-statement overhead
(one INSERT per record). Similarity query is identical speed —
NexusQL runs the full cosine + sort + limit in one pass.

---

## Performance

Measured on a Windows laptop, no SIMD, single thread:

| Operation | D=10,000 MAP | D=10,000 BSC |
|-|-|-|
| `hv_random` | ~10 µs | ~6 µs |
| `hv_atom` | ~12 µs | ~8 µs |
| `hv_bind` | ~10 µs | ~6 µs |
| `hv_bundle(N)` | ~N × 40 µs | ~N × 60 µs |
| `hv_similarity` | ~8 µs | ~5 µs |
| `hv_permute` | ~15 µs | ~60 µs (bit-by-bit, room to speed up) |
| Full-record encoding (~15 roles, ~50 tokens) | ~1 ms | ~1 ms |

Top-K similarity over 10k stored hypervectors runs in ~0.05 s (linear
scan + sort, no HNSW integration yet — see Roadmap).

---

## On-disk format

A stored hypervector is a `BLOB` that looks like:

```
[tag: 1 byte][dim: 8 bytes LE][body: variant-specific]
```

| Variant | Tag | Body size |
|-|-|-|
| BSC | 0 | `⌈D/64⌉ × 8` bytes (packed u64 chunks) |
| MAP | 1 | `⌈D/8⌉` bytes (1 bit per element, +1 → 1, −1 → 0) |

For D=10,000:
- BSC: 1,258 bytes per hypervector
- MAP: 1,258 bytes per hypervector

This means 10M hypervectors is ~12 GB — linear and predictable.
`Value::Blob` carries the bytes; the variant tag means you can even
store mixed-variant columns without schema tricks.

---

## Roadmap

**Landed** (`feat/hdc-expansion` branch):

- [x] BSC in a proper `variants/` module with the `HvOps` trait
- [x] BSC permutation + property-based tests
- [x] `Hypervector` as a true enum (runtime variant dispatch)
- [x] MAP variant with full algebra + tests
- [x] Seven `hv_*` SQL scalar functions (`hv_atom`, `hv_random`,
      `hv_bind`, `hv_bundle`, `hv_permute`, `hv_similarity`,
      `hv_variant`, `hv_dim`)
- [x] Real-world validation on a 2,363-record manufacturing workload

**Next up** (not yet shipped):

- [ ] **Aggregate `hv_bundle` over a group** — `SELECT class,
      hv_bundle(hv) FROM records GROUP BY class` for prototype
      construction. Today `hv_bundle` is scalar (N arguments), not an
      aggregate.
- [ ] **`hv_unbind` / unbinding** — `A` from `C = A ⊗ B` given `B`.
      BSC and MAP both have self-inverse binding, so
      `hv_bind(c, b)` already recovers `a`. An explicit
      `hv_unbind(c, b)` would make that intent clearer and handle
      variants where binding isn't self-inverse (HRR, future).
- [ ] **HNSW integration for `hv_similarity` ordering** — at the
      moment ORDER BY is a linear scan. For 1M+ hypervectors per
      table the existing HNSW index (used for `Vector` columns)
      should be reusable.
- [ ] **Dedicated `HYPERVECTOR(dim, 'variant')` column type** —
      currently hypervectors ride in `BLOB` columns. A proper type
      would enforce the (dim, variant) pair at insert time.
- [ ] **HRR / FHRR / SCH variants** — for continuous-value attributes
      where MAP's bipolar quantisation loses information. SCH in
      particular — sparse complex hypervectors — would integrate
      naturally with HNSW.
- [ ] **Optimize BSC permutation** from naive O(D) bit-copy to O(D/64)
      packed rotate. The property tests pin correctness so the switch
      is safe.

**Not on the roadmap** (out of scope for a database):

- Actual spiking-neuron / neuromorphic backends (Loihi 2, etc.) —
  research direction, not a DB feature.
- Resonator networks for factoring bound hypervectors. Add when a
  user hits the wall; nobody has yet.

---

## References

Foundational:

- Kanerva, P. (2009). "Hyperdimensional Computing: An Introduction to
  Computing in Distributed Representation with High-Dimensional
  Random Vectors." *Cognitive Computation* 1(2): 139–159.
- Karunaratne, G. et al. (2020). "In-memory hyperdimensional
  computing." *Nature Electronics*. DOI: 10.1038/s41928-020-0410-3.
- Kleyko, D. et al. (2022/2023). "Survey on Hyperdimensional
  Computing aka Vector Symbolic Architectures, Parts I and II."
  ACM Computing Surveys.

NexusQL integration plan (internal):

- `feat/hdc-expansion` branch commit history — commits `62fce96`
  through `b0a465f` walk through PR 1 → PR 6 in small steps.
