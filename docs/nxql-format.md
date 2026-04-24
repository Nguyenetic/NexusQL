# .nxql — NexusQL Portable Learning File Format

**Status**: Draft v0.1 (design document — no implementation)
**Last updated**: 2026-04-23
**Owner**: NexusQL core team

---

## 1. Motivation

Existing flashcard / spaced-repetition file formats each optimize for a single
ecosystem:

| Format | Strengths | Weaknesses |
|-|-|-|
| `.apkg` (Anki) | Rich scheduling, large ecosystem | Opaque SQLite blobs, no semantic layer, lossy across versions |
| `.csv` | Universal, trivial to diff | No media, no scheduling state, no schema |
| `.xlsx` | Familiar authoring, styles | Binary, poor round-trip, single-sheet assumptions |
| `.ankiweb` JSON exports | Human-readable | Non-standard, schedule data stripped |

Hibiki, Ullim, Shiori, and future NexusQL-powered learning apps need a single
portable file that:

1. **Round-trips cleanly** to and from `.apkg`, `.csv`, and `.xlsx` without
   silent data loss.
2. **Carries scheduling state** (SM-2 / FSRS parameters, review history)
   so a deck can migrate between tools without resetting progress.
3. **Embeds semantic indexes** (HDC hypervectors, ColBERT token embeddings,
   HNSW graph) so the NexusQL engine can serve similarity queries without
   re-indexing on import.
4. **Supports multiple card "models"** in one file (vocab, grammar, listening,
   kanji decomposition) the way Anki's note types do.
5. **Stays open** — the archive layout is zip, the catalog is a NexusQL
   snapshot, and every shape is documented here.

`.nxql` is the portable representation. The NexusQL engine is the authoritative
runtime.

---

## 2. File structure

A `.nxql` file is a **ZIP archive** with the following canonical layout:

```
deck.nxql
├── manifest.json              # Required. Envelope metadata.
├── catalog.nxqldb             # Required. NexusQL snapshot (SQLite-compatible page format).
├── notes/
│   ├── notes.parquet          # Required. Columnar card/note payloads.
│   └── schema.json            # Required. Field definitions per model.
├── schedule/
│   ├── reviews.parquet        # Optional. Per-review log (revlog equivalent).
│   └── state.parquet          # Required if any card has non-new state.
├── indexes/
│   ├── hnsw.bin               # Optional. Pre-built HNSW graph for semantic search.
│   ├── hdc.bin                # Optional. Hyperdimensional memory bundle.
│   └── colbert.bin            # Optional. Token-level embedding store.
├── media/
│   ├── <sha256>.<ext>         # Content-addressed media files.
│   └── index.json             # Map: logical name → sha256 → mime type.
├── models/
│   └── <model_id>.json        # Card model / note type definitions.
└── extensions/
    └── <vendor>/              # Vendor-specific payloads (Hibiki grammar notes, etc.).
```

**Rules:**

- All paths use forward slashes, UTF-8 encoded names.
- `manifest.json` MUST be the first entry in the archive so readers can
  stream-validate without decompressing the whole file.
- `catalog.nxqldb` is the **source of truth** when both it and the loose
  `notes/schedule` files are present — loose files exist for tools that cannot
  open the NexusQL page format.
- Compression: deflate (level 6) is the default. `indexes/*.bin` SHOULD be
  stored uncompressed (they are already entropy-dense).

---

## 3. Canonical card schema

Every card in a `.nxql` file is a row that conforms to a **model**. A model
declares the fields, the card templates (question/answer pairs derivable from
one note), and the scheduling algorithm. Models live in `models/<id>.json`; the
row payload lives in `notes/notes.parquet`.

### 3.1 Note row (Parquet)

| Column | Type | Required | Description |
|-|-|-|-|
| `note_id` | `binary(16)` | yes | UUIDv7 of the note. Stable across round-trips. |
| `model_id` | `binary(16)` | yes | FK to `models/<id>.json`. |
| `deck_path` | `string` | yes | Hierarchical path, `::` separator (e.g. `Japanese::JLPT::N5`). |
| `guid` | `string` | yes | Cross-ecosystem ID. Matches Anki `notes.guid` when imported. |
| `fields` | `map<string, string>` | yes | Field name → field value. HTML/Markdown allowed. |
| `tags` | `list<string>` | yes | Normalized tag names (lowercase, `-` separator). |
| `created_ms` | `int64` | yes | Unix ms. |
| `modified_ms` | `int64` | yes | Unix ms. |
| `checksum` | `binary(8)` | yes | First 8 bytes of SHA-256 over canonicalized `fields`. |
| `media_refs` | `list<string>` | no | SHA-256 hex of media referenced from `fields`. |
| `embedding_ref` | `binary(16)` | no | FK into `indexes/hnsw.bin` (null = not indexed). |
| `source` | `string` | no | Origin annotation (`"apkg:deck-name"`, `"hibiki:user-123"`). |

### 3.2 Card row (derived from note × template)

Cards are **generated** from notes at load time using the model's templates,
but their scheduling state MUST be persisted. That persistent slice lives in
`schedule/state.parquet`:

| Column | Type | Required | Description |
|-|-|-|-|
| `card_id` | `binary(16)` | yes | UUIDv7. |
| `note_id` | `binary(16)` | yes | FK. |
| `template_ord` | `uint16` | yes | Which template produced this card (0-indexed). |
| `algorithm` | `string` | yes | `"sm2"` \| `"fsrs-v5"` \| `"leitner"`. |
| `state` | `string` | yes | `"new"` \| `"learning"` \| `"review"` \| `"relearning"` \| `"suspended"` \| `"buried"`. |
| `due_ms` | `int64` | yes | Unix ms when next due (0 if new). |
| `interval_s` | `int64` | yes | Current interval in seconds (negative = learning step). |
| `ease` | `float32` | yes | SM-2 ease factor (default 2.5). |
| `reps` | `uint32` | yes | Lifetime review count. |
| `lapses` | `uint32` | yes | Times transitioned to relearning. |
| `stability` | `float32` | no | FSRS stability parameter. |
| `difficulty` | `float32` | no | FSRS difficulty parameter. |
| `last_reviewed_ms` | `int64` | no | Unix ms of last review. |
| `flags` | `uint32` | yes | Bit-packed: `0x1` marked, `0x2` leech, `0x4` auto-suspended. |

### 3.3 Review log (`schedule/reviews.parquet`)

| Column | Type | Description |
|-|-|-|
| `review_id` | `binary(16)` | UUIDv7 of this review event. |
| `card_id` | `binary(16)` | FK. |
| `reviewed_at_ms` | `int64` | Unix ms. |
| `rating` | `uint8` | 1=again, 2=hard, 3=good, 4=easy. |
| `prev_interval_s` | `int64` | Interval before the review. |
| `new_interval_s` | `int64` | Interval after the review. |
| `prev_ease` | `float32` | Ease before. |
| `new_ease` | `float32` | Ease after. |
| `duration_ms` | `uint32` | Time spent (capped at 60000 to match Anki). |
| `type` | `uint8` | 0=learn, 1=review, 2=relearn, 3=filtered, 4=manual, 5=rescheduled. |

### 3.4 Model definition (`models/<id>.json`)

```json
{
  "model_id": "018f9c8a-1c66-7a81-9a0b-c17e9d4a1f12",
  "name": "Japanese::Vocabulary",
  "version": 3,
  "created_ms": 1735689600000,
  "modified_ms": 1745971200000,
  "fields": [
    { "name": "word",      "type": "string", "required": true,  "indexed": "ft,hnsw" },
    { "name": "reading",   "type": "string", "required": false, "indexed": "ft" },
    { "name": "meaning",   "type": "string", "required": true,  "indexed": "ft,colbert" },
    { "name": "audio",     "type": "media",  "required": false, "mime": "audio/mpeg" },
    { "name": "jlpt",      "type": "enum",   "values": ["N5","N4","N3","N2","N1"] }
  ],
  "templates": [
    {
      "ord": 0,
      "name": "Recognition",
      "qfmt": "{{word}}",
      "afmt": "{{FrontSide}}<hr>{{reading}}<br>{{meaning}}{{#audio}}<br>[sound:{{audio}}]{{/audio}}"
    },
    {
      "ord": 1,
      "name": "Production",
      "qfmt": "{{meaning}}",
      "afmt": "{{FrontSide}}<hr>{{word}} ({{reading}})"
    }
  ],
  "css": ".card { font-family: 'Noto Sans JP'; font-size: 22px; }",
  "scheduler": {
    "algorithm": "fsrs-v5",
    "parameters": [0.4, 0.6, 2.4, 5.8, 4.93, 0.94, 0.86, 0.01, 1.49, 0.14, 0.94, 2.18, 0.05, 0.34, 1.26, 0.29, 2.61]
  },
  "semantic": {
    "hdc_dim": 10000,
    "colbert_tokens": true,
    "hnsw_metric": "cosine"
  }
}
```

---

## 4. Manifest

The manifest is the envelope every reader inspects first. A fully-realized
example:

```json
{
  "$schema": "https://nexusql.dev/schemas/nxql-1.0.json",
  "format_version": "1.0.0",
  "nexusql_min_version": "2.0.0",
  "file_id": "018f9c8b-4c12-7f3a-8e7e-aa1bd3c4e001",
  "created_ms": 1745971200000,
  "modified_ms": 1745971200000,
  "creator": {
    "app": "hibiki",
    "version": "0.7.3",
    "user_hint": "john.nguyen9727@gmail.com"
  },
  "title": "JLPT N5 — Vocabulary & Grammar",
  "description": "Core 800 words plus 60 grammar patterns for JLPT N5.",
  "language": { "target": "ja", "native": "en" },
  "decks": [
    { "path": "JLPT::N5::Vocab",   "card_count": 812,  "new": 812,  "due_today": 0 },
    { "path": "JLPT::N5::Grammar", "card_count": 60,   "new": 60,   "due_today": 0 }
  ],
  "models": [
    { "model_id": "018f9c8a-1c66-7a81-9a0b-c17e9d4a1f12", "name": "Japanese::Vocabulary", "version": 3 },
    { "model_id": "018f9c8a-1c66-7a81-9a0b-c17e9d4a2099", "name": "Japanese::Grammar",    "version": 1 }
  ],
  "stats": {
    "note_count": 872,
    "card_count": 1684,
    "review_count": 0,
    "media_count": 412,
    "media_bytes": 18274910
  },
  "indexes": {
    "hnsw":    { "present": true,  "dim": 768,   "entries": 872, "metric": "cosine" },
    "hdc":     { "present": true,  "dim": 10000, "entries": 872 },
    "colbert": { "present": false }
  },
  "provenance": [
    { "source": "apkg",   "file": "jlpt-n5.apkg",          "imported_ms": 1745971000000, "sha256": "ab3c…" },
    { "source": "xlsx",   "file": "grammar-patterns.xlsx", "imported_ms": 1745971100000, "sha256": "c9f1…" }
  ],
  "checksums": {
    "algorithm": "sha256",
    "catalog.nxqldb":  "4f8b…",
    "notes/notes.parquet": "9a22…",
    "schedule/state.parquet": "e117…"
  },
  "signature": {
    "algorithm": "ed25519",
    "public_key_hex": "…",
    "signature_hex": "…"
  }
}
```

`signature` is **optional**. When present, every file listed under
`checksums` MUST be covered so that tamper-evidence works end-to-end.

---

## 5. Conversion rules

### 5.1 `.apkg` ↔ `.nxql`

| .apkg concept | .nxql equivalent | Notes |
|-|-|-|
| `col.decks` (JSON) | `manifest.decks[]` + `note_row.deck_path` | Anki's `::` hierarchy is preserved verbatim. |
| `col.models` (JSON) | `models/<id>.json` | Templates, fields, CSS map 1:1. Model ID is rehashed to UUIDv7 using the epoch-ms Anki ID as the timestamp component. |
| `notes.flds` (0x1f-separated) | `note_row.fields` (map) | Split on 0x1f and keyed by model field names. |
| `notes.tags` | `note_row.tags` | Anki uses space-separated with leading/trailing spaces; we strip and normalize case. |
| `cards.{type,queue,due,ivl,factor,reps,lapses,left}` | `schedule/state.parquet` | See table below. |
| `revlog` | `schedule/reviews.parquet` | `ease` → `rating`, negative `ivl` means seconds. |
| `media` JSON map | `media/index.json` + content-addressed files | Original filenames preserved in `index.json`. |
| `graves` | Not preserved | Deletion tombstones are an Anki-sync concern and have no meaning in a portable file. **(lossy)** |

#### Scheduling field mapping

| Anki field | .nxql field | Transform |
|-|-|-|
| `cards.type` (0–3) | `state` | 0→new, 1→learning, 2→review, 3→relearning. |
| `cards.queue` (-3 to 4) | `state` override | -1→suspended, -2/-3→buried. |
| `cards.due` | `due_ms` | If type=new: preserve as sort key in `flags` high bits. If learning: treat as unix-seconds, convert to ms. If review: `(col.crt + due * 86400) * 1000`. |
| `cards.ivl` | `interval_s` | Positive days → seconds (`* 86400`). Negative already seconds → negate and keep. |
| `cards.factor` (permille) | `ease` | `factor / 1000.0` (e.g. 2500 → 2.5). |
| `cards.reps`, `cards.lapses` | `reps`, `lapses` | 1:1. |
| `cards.left` | Dropped **(lossy, minor)** | Represents "reps remaining today"; recomputed on next session. |
| `cards.odue`, `cards.odid` | Dropped **(lossy)** | Filtered-deck state is Anki-specific; cards are re-homed to their original deck. |

#### Documented losses going `.apkg` → `.nxql`

1. Filtered deck membership (`odid`, `odue`).
2. Graveyard rows.
3. Per-sync USN (update sequence numbers).
4. `cards.left` fine-grained learning step counter (state is preserved, but the
   "steps remaining today" counter is dropped; FSRS does not use it).
5. Anki-specific custom scheduler JS (`col.conf.customSchedulerCode`).

#### Documented losses going `.nxql` → `.apkg`

1. HDC and ColBERT indexes (no Anki equivalent).
2. FSRS-v5 parameters beyond what Anki 23.10+ understands.
3. UUIDv7 note/card IDs are remapped to Anki epoch-ms IDs; originals are kept
   in a `nxql.original_id` tag for reverse mapping.
4. Extension payloads under `extensions/<vendor>/`.

### 5.2 `.csv` ↔ `.nxql`

CSV is treated as **one model, one deck, no media, no schedule.**

**CSV → .nxql:**
1. First row MUST be a header. Column names become model field names.
2. A single-template "Recognition" model is synthesized with `qfmt = {{<first_col>}}` and `afmt = {{FrontSide}}<hr>{{<second_col>}}`.
3. Every row becomes a new note in state `new`. `guid` is
   `sha256(model_id || canonicalized_row)[:16]`.
4. Reserved columns (case-insensitive): `tags`, `deck`, `note_id`, `model_id`,
   `due_ms`, `interval_s`, `ease`, `reps`, `lapses`, `state`. If present, they
   populate schedule state instead of becoming fields.
5. Media references are **not inferred** from CSV. If a cell looks like a URL
   or path, it is stored as literal text.

**.nxql → CSV:**
1. Only works if exactly one model is present. Multi-model files error out with
   a suggestion to use `.xlsx` instead.
2. Field columns come first (in model order), then the reserved columns.
3. Media fields are replaced with their SHA-256 hex string; a sibling
   `media/` directory is dropped alongside the CSV. **(lossy if media present)**.
4. HTML/Markdown in fields is preserved verbatim.

**Documented CSV losses:** templates, CSS, media binaries (only the digest
remains), review history, HNSW/HDC/ColBERT indexes, multi-deck/multi-model
structure.

### 5.3 `.xlsx` ↔ `.nxql`

XLSX is the authoring-friendly path. One workbook = one `.nxql`.

| Sheet name | Contents |
|-|-|
| `_manifest` | Key/value pairs corresponding to `manifest.json` top-level fields. |
| `_models` | One row per model, with columns `model_id`, `name`, `version`, `templates_json`, `css`, `fields_json`. |
| `<model_name>` | One sheet per model; columns are the model's fields plus reserved `deck`, `tags`, schedule columns. |
| `_reviews` | Optional. Mirrors `schedule/reviews.parquet` schema. |
| `_media` | Optional. `name`, `sha256`, `mime`, `bytes_base64`. Inlined so the workbook stays self-contained. |

**XLSX → .nxql:**
- Hidden sheets (starting with `_`) are metadata; visible sheets are notes.
- Empty cells in required fields → row rejected with a per-row error report
  emitted into `extensions/nxql/import-errors.json`.

**XLSX → .nxql losses:** Cell styles, formulas, and charts are discarded.
Only values are preserved.

**.nxql → XLSX losses:** Binary indexes are dropped (they would bloat the
workbook and XLSX is not a runtime format). Media over 2 MB each is replaced
with a relative path reference and bundled in a `media/` sidecar folder.

---

## 6. Caching strategy via the NexusQL engine

`.nxql` is a portable archive; **NexusQL is the runtime.** On first open, a
reader extracts the archive into a cache directory and uses NexusQL to serve
queries.

### 6.1 Cache layout

```
~/.cache/nexusql/nxql/<file_id>/
├── catalog.nxqldb            # Opened as a NexusQL database
├── media/                    # Extracted media (content-addressed)
├── hnsw.bin                  # Memory-mapped
├── hdc.bin                   # Memory-mapped
├── colbert.bin               # Memory-mapped
└── .meta.json                # { source_path, source_mtime, source_sha256, extracted_at_ms }
```

### 6.2 Open-file algorithm

1. Compute `sha256` of the `.nxql` file.
2. If `~/.cache/nexusql/nxql/<file_id>/` exists and `.meta.json` matches
   both `source_path` and `source_sha256`, **reuse the cache** and open the
   NexusQL catalog directly. This is the hot path — zero decompression.
3. Otherwise, extract the archive, open `catalog.nxqldb` via the NexusQL
   engine, register the HNSW/HDC/ColBERT indexes, and write `.meta.json`.
4. If the archive lacks pre-built indexes but the catalog contains notes that
   declare `indexed` fields in their model, **build indexes lazily on first
   semantic query**, then persist them back into the cache so subsequent
   opens are warm.

### 6.3 Read-only vs. read-write

- Default open mode is **read-only** — mutations go to a session-scoped
  overlay database in the cache (`catalog.overlay.nxqldb`) and are merged on
  explicit save.
- `nxql.save(path)` re-packages: it serializes the merged catalog, regenerates
  Parquet exports, recomputes checksums, and writes a new archive atomically
  via temp-file-rename.

### 6.4 Eviction

Caches are LRU-evicted when the total nxql cache exceeds 2 GB or when
individual entries are older than 30 days and have not been touched. Never
evict an entry whose source file still exists at its registered path — the
cache key is `(source_sha256, source_path)`.

---

## 7. Versioning and migration

### 7.1 Version fields

- `manifest.format_version` — semver of the `.nxql` format itself.
- `manifest.nexusql_min_version` — minimum engine version that can open this
  file without migration.
- Each `models/<id>.json` has its own `version` — a model can evolve
  independently (e.g. adding a new field) without bumping the file format.

### 7.2 Compatibility rules

| Scenario | Behavior |
|-|-|
| Reader's format major == file's format major | Open normally. |
| Reader's format major > file's format major | Migrate in place (see below). |
| Reader's format major < file's format major | Refuse to open; direct the user to upgrade. |
| Field added to a model (minor bump) | Old readers see `null`; new readers see the field. |
| Field removed from a model (major bump) | Writers MUST retain the old column in `notes.parquet` under a `_deprecated__<name>` prefix for one major version. |

### 7.3 Migration mechanism

Migrations live under `src/nxql/migrations/v<from>_to_v<to>.rs` in the NexusQL
crate. Each migration is a pure function `(OldFile) -> Result<NewFile>` that:

1. Opens the old archive read-only.
2. Produces a new archive in a temp directory.
3. Validates the new archive against the current schema.
4. Atomically renames over the old archive (a `.bak` copy is written next to
   the original before replacement, deleted after 7 days).

The engine ships with a `nexusql migrate <file.nxql>` CLI subcommand that
chains all required migrations and reports the diff.

### 7.4 Deprecation policy

- Format-level deprecations are announced one minor version before removal.
- Removed fields are listed in `CHANGELOG.md` under `nxql-format`.
- Migrations for a given major are maintained for at least 18 months after
  the next major ships.

---

## 8. Security model

- **No code execution from `.nxql` files.** Template rendering is
  sandboxed: only safe mustache-like substitution, no JavaScript, no
  `<script>` tags escape rendering.
- **Media MIME sniffing** is done via the declared mime in `media/index.json`
  AND magic-byte validation. A mismatch fails the load.
- **Zip-slip protection**: any extracted path that resolves outside the
  designated cache directory is rejected.
- **Signature verification** (when `manifest.signature` is present) is done
  before any file is extracted. A failed signature is a hard error unless the
  caller explicitly passes `--unverified`.
- **Size limits**: readers MUST enforce per-entry uncompressed size limits
  (default 500 MB) and total archive size limits (default 5 GB) to prevent
  zip-bomb attacks.

---

## 9. Implementation roadmap

### Phase 1 — Reader (MVP)

- Parse `manifest.json`, validate schema.
- Open `catalog.nxqldb` via the existing NexusQL engine.
- Iterate notes and schedule rows via the engine (no Parquet path yet — read
  everything from the catalog).
- Expose a Rust API: `NxqlFile::open(path) -> Result<NxqlFile>` with
  `.decks()`, `.notes()`, `.due_cards(at)`.
- JNI + Node bindings that mirror the Rust surface.
- **Deliverables**: `src/nxql/reader.rs`, `bindings/android/nxql.rs`,
  `bindings/node/nxql.ts`, reader tests against hand-crafted fixtures.

### Phase 2 — Writer

- Serialize a NexusQL catalog into the archive layout.
- Emit Parquet exports alongside the catalog (dual-path redundancy).
- Content-address all media, dedupe across decks.
- Emit `manifest.checksums` and optionally sign with ed25519.
- `nexusql pack <dir> -o <file.nxql>` CLI.
- **Deliverables**: `src/nxql/writer.rs`, round-trip tests (read-then-write
  produces byte-identical output when the source has no mutations).

### Phase 3 — Converters

- `.apkg` importer (zip → read `collection.anki2` via NexusQL SQLite compat
  layer → translate schema).
- `.apkg` exporter with "Support older Anki versions" flag.
- `.csv` importer/exporter.
- `.xlsx` importer/exporter (using the `calamine` + `rust_xlsxwriter` crates).
- A per-converter lossiness report stored under
  `extensions/nxql/conversion-report.json` on import.
- **Deliverables**: `src/nxql/convert/{apkg,csv,xlsx}.rs`, end-to-end
  round-trip fixtures for each path (apkg → nxql → apkg must preserve all
  non-lossy fields).

### Phase 4 — Semantic indexes

- Persist HNSW graph directly to `indexes/hnsw.bin` in a stable on-disk
  layout (document the layout in `docs/hnsw-ondisk.md`).
- Persist HDC bundles and ColBERT token stores.
- Lazy-build indexes on first semantic query for files that shipped without
  them.
- `nexusql nxql reindex <file>` CLI to force a rebuild.
- **Deliverables**: `src/nxql/indexes/{hnsw,hdc,colbert}.rs`, memory-mapped
  loaders, benchmarks demonstrating cold-open < 100 ms for a 10k-note deck.

### Phase 5 — Ecosystem

- Hibiki integration: swap Supabase-backed favorites/notes for a `.nxql`
  export/import.
- Ullim (Korean) parity.
- Shiori (Android) offline-first nxql storage.
- Public schema registry at `nexusql.dev/schemas/` for cross-vendor tooling.
- A reference "nxql-viewer" web app (Wasm build of the reader) so users can
  inspect any `.nxql` file without installing anything.
- **Deliverables**: integration PRs against Hibiki and Ullim, Android JNI
  adapter in Shiori, wasm viewer published to `nexusql.dev/viewer`.

---

## 10. Open questions

1. **Should scheduling state be optional on import?** Today the spec requires
   it when any card is not `new`. Forcing state may add friction for
   "import these words as a fresh deck" flows — consider a
   `?mode=reset` query-style hint on import.
2. **Is Parquet the right columnar format?** Arrow IPC is simpler, Parquet is
   more universal. Leaning Parquet for cross-tool inspectability.
3. **Do we need per-note ACLs?** Probably not for v1 — `.nxql` is a
   single-owner file. Multi-user sync is a separate layer.
4. **Provenance chaining.** Each import adds to `manifest.provenance[]`; do we
   truncate or keep forever? Recommendation: keep forever but compress older
   than 5 entries into a `.provenance.history.json` extension file.

---

## 11. References

- Anki `.apkg` internals: Anki-Android wiki "Database Structure", brandur.org "Anki's .apkg".
- FSRS-v5 algorithm: `open-spaced-repetition/fsrs4anki` specification.
- NexusQL engine: `README.md`, `docs/architecture/neural-technologies.md`.
- Parquet format: Apache Parquet 2.9 specification.
- UUIDv7: RFC 9562 §5.7.
