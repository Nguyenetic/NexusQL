# Geometry and small-corpus SQL validation

Validated on 2026-10-13, Windows, Rust 1.98.0, Python 3.12.10.
Baseline: clean `9a71d56`. Implementation: local commits through `8e98055`.
No push, PR, or merge was performed.

## Shipped

- GEOMETRY values, WKT, optional SRID metadata, all 14 requested spatial functions,
  and `ST_SRID`. Geometry schema and values survive reopening the database.
- Real rstar R-trees for geometry GiST indexes; range GiST is retained for other
  columns. SQL filters and INNER/LEFT joins retrieve candidates and recheck exact
  predicates. INSERT/bulk insert maintain entries; UPDATE/DELETE rebuild them.
- Native Python WKT output and verified parameterized execution, built with
  `maturin develop` and tested with pytest. README source-build instructions updated.
- Lightweight character n-gram ColBERT MaxSim scoring and one-shot MAP HDC models
  stored as ordinary BLOBs, with SQL learning, prediction, merging, and confidence.

See [geometry usage/design](../guides/geometry.md) and
[small-corpus usage/design](../guides/small-corpus-ai.md).

## Baseline versus final

| Check | Untouched baseline | Final |
| --- | --- | --- |
| `cargo build` | Pass | Pass |
| `cargo test` | Existing compilation failures; cannot run entire suite | Still blocked by existing compilation failures |
| Runnable library/integration targets | 474 passed, 4 failed | 494 passed, same 4 failed |
| New feature tests with `--no-default-features` | Not present | 20 passed |
| Formatting on changed Rust files | Not applicable | Pass, 30 files |
| `cargo clippy` (report-only) | Not measured | Exit 0, 154 warnings |
| Python `maturin develop` | Not measured | Pass |
| Python `pytest -q` | No binding tests present | 11 passed |

The runnable comparison uses `cargo test --lib --no-fail-fast` with each top-level
integration target except the already uncompilable `where_clause_test` explicitly
selected via `--test`. Final totals: 466 library, 8 existing integration, 4 geometry,
7 spatial index, 4 lightweight ColBERT, and 5 HDC classifier tests passed.

Formatting used `cargo fmt --check` with a temporary manifest listing exactly the
30 changed Rust files as targets and `skip_children=true`; untouched files were
not reformatted. Build, full test attempt, runnable comparison, and Clippy ran in
the requested order. Python was rebuilt and tested after the final Rust checks.

## Existing failures left unchanged

Baseline compilation failures were observed in these targets:

| Target | Existing cause |
| --- | --- |
| `tests/where_clause_test.rs` | Three calls omit the required QueryParams argument |
| `examples/lsm_test.rs` | Constructor call omits the required second argument |
| `examples/transaction_demo.rs` | Five type mismatches |
| `examples/auth_system_demo.rs` | Four type mismatches |
| `examples/hyper_optimized_test.rs` | Type mismatch |
| `examples/ultra_optimized_test.rs` | Type mismatch |
| `examples/fast_100k_test.rs` | Type mismatch |
| `examples/week10_features_demo.rs` | 37 errors: unresolved `nexus`, invalid println arguments/repeat usage |

Cargo stops compiling after failures, so the subset printed by a full test run
varies with parallel build scheduling. None of these files was changed.

The same four `integration_test` tests fail before and after:
`test_initialization`, `test_basic_query`, `test_colbert_routing`, and
`test_hdc_routing`. They share `./test.nexus`, causing Windows file-lock failures;
which test acquires the lock varies. Other exposed existing issues are the
missing `test` table and unsupported `USING hdc` query syntax. These were not fixed.

## Acceptance evidence and limits

The three-zone/six-device test has hand-computed results, including a point on a
shared edge and one outside every zone. Contains excludes the edge point;
Intersects includes it in both adjacent zones. Hole and distance tests verify
that bounding-box candidates still receive exact checks.

The 10,000-point/500-polygon join produces the same 6,000 rows with and without
the index. It asserts 10,000 real R-tree searches, a multi-level tree, and fewer
than 25,000 candidates out of 5,000,000 possible pairs. Maintenance tests cover
inserts, bulk inserts, updates, deletes, replacement, rollback, and reopening.

No requested priority was left unfinished. Limits: planar 2D only, no SRID
transforms, right-side index probes for joins, and exact sorting rather than an
indexed KNN path for nearest-neighbor queries. Invalid scalar arguments follow
the existing NULL convention. HDC confidence is a similarity margin, not a
probability. Existing disk transaction behavior writes DML before COMMIT; the
classifier inherits that behavior. See the usage guides for input limits.
