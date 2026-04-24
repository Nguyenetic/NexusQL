# .nxql — NexusQL Portable Learning File Format

**Status**: Draft v0.1 (design document — no implementation)
**Last updated**: 2026-04-24
**Owner**: NexusQL core team
**Audience**: Future implementation teammates (reader/writer, converters, Shiori/Hibiki integrators)

---

## 1. Motivation

Flashcard and spaced-repetition tooling is fragmented. Anki's `.apkg` is the
de-facto standard but bakes all state into opaque SQLite; CSV and XLSX travel
well but discard scheduling and media; no portable format carries semantic
indexes. Learners end up locked into one tool or losing progress when they
switch.

`.nxql` is a **portable, SQLite-openable, NexusQL-native** learning content
file. One file opens natively in Shiori (via the NexusQL engine), imports
cleanly into Anki (via its embedded SQLite mirror), and flattens to CSV/XLSX
for spreadsheet users. It carries schedule state (SM-2 + FSRS), media, and an
optional HNSW vector index for semantic similarity.

Design goals:

1. **Native-fast for NexusQL**: open with zero copy, zero re-indexing. The
   on-disk NexusQL LSM + WAL *is* the primary store.
2. **SQLite-openable**: any tool that can read `.anki2` can query the embedded
   `compat/anki.sqlite` mirror without running NexusQL.
3. **Self-describing**: `manifest.json` declares everything present and its
   checksums.
4. **Round-trips** to `.apkg`, `.csv`, `.tsv`, and `.xlsx` with documented
   losses.
5. **Open and inspectable**: every format is text, SQLite, or a documented
   binary layout.

---

## 2. File structure

A `.nxql` file is a **ZIP archive** (deflate, level 6) with this canonical
layout:

```
deck.nxql
├── manifest.json              # Required. Envelope metadata.
├── nexusql/                   # Required. Native NexusQL storage (primary source of truth).
│   ├── MANIFEST               # NexusQL LSM manifest (current SSTables + edits)
│   ├── WAL/
│   │   └── 000001.wal         # Write-ahead log segments
│   ├── sst/
│   │   ├── L0/                # Level-0 SSTables (newest)
│   │   ├── L1/
│   │   └── L2/
│   └── CATALOG.json           # Table schemas, sequences, index metadata
├── cards/                     # Optional. Interop mirror — one JSON per card.
│   ├── 018f9c8b-....json
│   └── index.json             # card_id → relative path lookup
├── media/                     # Content-addressed media files.
│   ├── <sha256>.<ext>
│   └── index.json             # logical name → sha256 → mime
├── vectors.hnsw               # Optional. Prebuilt HNSW graph for semantic search.
├── compat/                    # Fallback exports for non-NexusQL tools.
│   ├── anki.sqlite            # Anki-schema SQLite mirror (schema matches .anki2)
│   ├── export.csv             # Flat CSV (one model assumed, lossy on multi-model)
│   ├── export.tsv             # Tab-separated for Anki import
│   └── export.xlsx            # Optional. Excel workbook, one sheet per deck.
└── extensions/
    └── <vendor>/              # Vendor-specific payloads (Hibiki grammar, Ullim radicals, etc.)
```

### 2.1 Archive rules

- Paths use forward slashes, UTF-8 names.
- `manifest.json` MUST be the first entry in the archive so readers can
  stream-validate without decompressing the whole file.
- `nexusql/**` is stored with `STORED` (no compression) — the LSM already
  writes compressed blocks, and mmap requires aligned data.
- `vectors.hnsw`, `compat/anki.sqlite`, and media files are also `STORED`.
- Everything else uses deflate-6.
- Total uncompressed cap: **5 GB**. Per-entry cap: **500 MB**. Readers MUST
  enforce both to prevent zip-bomb attacks.

### 2.2 Source of truth

`nexusql/` is the **primary**, authoritative store. The other directories are
derived views:

| Layer | Role | Written by |
|-|-|-|
| `nexusql/` | Source of truth. Opened read-write. | Writer on save. |
| `cards/*.json` | Human-readable, diff-friendly mirror. | Writer on save. |
| `compat/anki.sqlite` | SQLite interop mirror. | Writer on save, via the converter. |
| `compat/export.{csv,tsv,xlsx}` | Flat exports. | Writer on save, if requested. |
| `vectors.hnsw` | Prebuilt index for instant semantic search. | Writer on save, if any card has an embedding. |

When a reader encounters inconsistency across layers, `nexusql/` wins. The
older-reader rule in §7 is the sole exception.

---

## 3. Canonical card schema

Every card round-trips via this canonical shape. It lives in three places
(NexusQL tables, `cards/*.json`, and `compat/anki.sqlite`) and all three MUST
agree at save time.

### 3.1 Card JSON (canonical)

```json
{
  "id": "018f9c8b-4c12-7f3a-8e7e-aa1bd3c4e001",
  "deck": "JLPT::N5::Vocab",
  "type": "basic",
  "front": "猫",
  "back": "ねこ — cat",
  "hint": null,
  "extra": "<audio controls src=\"[sound:neko.mp3]\"></audio>",
  "tags": ["jlpt-n5", "animals", "noun"],
  "created_at": 1735689600000,
  "updated_at": 1745971200000,
  "media_refs": [
    "a3f9c7d2e8b1...neko.mp3"
  ],
  "audio_ref": "a3f9c7d2e8b1...neko.mp3",
  "vector": null,
  "srs": {
    "algorithm": "fsrs-v5",
    "state": "review",
    "last_reviewed": 1745884800000,
    "next_due": 1746316800000,
    "interval": 432000,
    "ease": 2.5,
    "lapses": 1,
    "reps": 12,
    "stability": 14.7,
    "difficulty": 5.2
  }
}
```

### 3.2 Field reference

| Field | Type | Required | Notes |
|-|-|-|-|
| `id` | UUIDv7 string | yes | Stable across round-trips. Generated at first import. |
| `deck` | string | yes | `::`-separated hierarchy, matches Anki. |
| `type` | enum | yes | `basic`, `reversed`, `cloze`, `image_occlusion`. Unknown types MUST be rejected by strict readers and preserved as-is by tolerant readers. |
| `front` | text/html | yes | HTML allowed. Sandboxed at render time (§8). |
| `back` | text/html | yes | Same rules as `front`. |
| `hint` | text/html | no | Optional prompt shown on the question side. |
| `extra` | text/html | no | Shown on the answer side, below `back`. |
| `tags` | string[] | yes | Normalized: lowercase, `-` separator, deduped. |
| `created_at` | int64 ms | yes | Unix ms since epoch. |
| `updated_at` | int64 ms | yes | Unix ms. |
| `media_refs` | string[] | no | SHA-256 hex digests of files in `media/`. |
| `audio_ref` | string | no | Single primary audio file (native TTS or bundled). Subset of `media_refs`. |
| `vector` | float32[] or null | no | Embedding used to populate `vectors.hnsw`. Omit if none. |
| `srs` | object | yes | See §3.3. |

### 3.3 SRS sub-schema

Carries **both** SM-2 and FSRS state so round-trips are lossless in either
direction:

| Field | Type | Required | Notes |
|-|-|-|-|
| `algorithm` | `"sm2"` \| `"fsrs-v5"` \| `"leitner"` | yes | Which algorithm produced the current state. |
| `state` | `"new"` \| `"learning"` \| `"review"` \| `"relearning"` \| `"suspended"` \| `"buried"` | yes |
| `last_reviewed` | int64 ms | no | null if never reviewed. |
| `next_due` | int64 ms | yes | 0 for `new`. |
| `interval` | int64 s | yes | Current interval in seconds. Negative = learning step. |
| `ease` | float32 | yes | SM-2 ease. Default 2.5. |
| `lapses` | uint32 | yes | Times bounced to relearning. |
| `reps` | uint32 | yes | Lifetime review count. |
| `stability` | float32 | no | FSRS stability (omit if algorithm=`sm2`). |
| `difficulty` | float32 | no | FSRS difficulty (omit if algorithm=`sm2`). |

### 3.4 NexusQL tables (source of truth)

The `nexusql/` store exposes these tables via `CATALOG.json`:

```sql
CREATE TABLE cards (
  id           BYTEA(16)     PRIMARY KEY,
  deck         TEXT          NOT NULL,
  type         TEXT          NOT NULL,
  front        TEXT          NOT NULL,
  back         TEXT          NOT NULL,
  hint         TEXT,
  extra        TEXT,
  tags         TEXT[]        NOT NULL,
  created_at   BIGINT        NOT NULL,
  updated_at   BIGINT        NOT NULL,
  audio_ref    TEXT,
  vector       FLOAT4[]
);

CREATE TABLE card_media (
  card_id   BYTEA(16) NOT NULL REFERENCES cards(id),
  media_sha TEXT      NOT NULL,
  PRIMARY KEY (card_id, media_sha)
);

CREATE TABLE card_srs (
  card_id        BYTEA(16) PRIMARY KEY REFERENCES cards(id),
  algorithm      TEXT      NOT NULL,
  state          TEXT      NOT NULL,
  last_reviewed  BIGINT,
  next_due       BIGINT    NOT NULL,
  interval       BIGINT    NOT NULL,
  ease           FLOAT4    NOT NULL,
  lapses         INTEGER   NOT NULL,
  reps           INTEGER   NOT NULL,
  stability      FLOAT4,
  difficulty     FLOAT4
);

CREATE TABLE reviews (
  id             BYTEA(16) PRIMARY KEY,
  card_id        BYTEA(16) NOT NULL REFERENCES cards(id),
  reviewed_at    BIGINT    NOT NULL,
  rating         SMALLINT  NOT NULL,
  prev_interval  BIGINT    NOT NULL,
  new_interval   BIGINT    NOT NULL,
  prev_ease      FLOAT4    NOT NULL,
  new_ease       FLOAT4    NOT NULL,
  duration_ms    INTEGER   NOT NULL,
  type           SMALLINT  NOT NULL
);

CREATE TABLE media (
  sha256     TEXT PRIMARY KEY,
  mime       TEXT NOT NULL,
  bytes      BIGINT NOT NULL,
  original   TEXT
);

CREATE INDEX ix_cards_deck     ON cards (deck);
CREATE INDEX ix_cards_srs_due  ON card_srs (state, next_due);
CREATE INDEX ix_reviews_card   ON reviews (card_id, reviewed_at);
CREATE HNSW  ix_cards_vector   ON cards (vector) WITH (metric = 'cosine');
```

The HNSW index is persisted to `vectors.hnsw` when the file is written; when
absent, semantic queries fall back to a linear scan over `cards.vector`.

---

## 4. Manifest

Fully-realized example:

```json
{
  "$schema": "https://nexusql.dev/schemas/nxql-1.0.json",
  "format_version": "1.0.0",
  "nexusql_min_version": "2.0.0",
  "file_id": "018f9c8b-4c12-7f3a-8e7e-aa1bd3c4e001",
  "creator": {
    "app": "hibiki",
    "version": "0.7.3",
    "user_hint": "john.nguyen9727@gmail.com"
  },
  "created_at": 1735689600000,
  "updated_at": 1745971200000,
  "title": "JLPT N5 — Vocabulary & Grammar",
  "description": "Core 800 words plus 60 grammar patterns for JLPT N5.",
  "content_language": { "target": "ja", "native": "en" },
  "card_count": 1684,
  "note_count": 872,
  "media_count": 412,
  "media_bytes": 18274910,
  "decks": [
    { "path": "JLPT::N5::Vocab",   "card_count": 1624, "new": 812, "due_today": 0 },
    { "path": "JLPT::N5::Grammar", "card_count":   60, "new":  60, "due_today": 0 }
  ],
  "features": {
    "srs_algorithms": ["fsrs-v5", "sm2"],
    "types_used": ["basic", "reversed", "cloze"],
    "has_media": true,
    "has_vectors": true,
    "vector_dim": 768,
    "vector_metric": "cosine"
  },
  "layers_present": {
    "nexusql": true,
    "cards_json": true,
    "compat_anki_sqlite": true,
    "compat_csv": true,
    "compat_tsv": true,
    "compat_xlsx": false,
    "vectors_hnsw": true
  },
  "provenance": [
    { "source": "apkg", "file": "jlpt-n5.apkg",          "imported_at": 1745971000000, "sha256": "ab3c..." },
    { "source": "xlsx", "file": "grammar-patterns.xlsx", "imported_at": 1745971100000, "sha256": "c9f1..." }
  ],
  "checksums": {
    "algorithm": "sha256",
    "nexusql/MANIFEST":        "4f8b...",
    "cards/index.json":        "9a22...",
    "compat/anki.sqlite":      "e117...",
    "compat/export.csv":       "7d4a...",
    "vectors.hnsw":            "1e02...",
    "media/index.json":        "5bc3..."
  },
  "signature": {
    "algorithm": "ed25519",
    "public_key_hex": "...",
    "signature_hex":  "..."
  }
}
```

`signature` is optional. When present, every file under `checksums` MUST be
covered — tamper-evidence is all-or-nothing.

---

## 5. Conversion rules

Every converter emits an `extensions/nxql/conversion-report.json` into the
archive on import so downstream tools can see exactly what was lost.

### 5.1 `.apkg → .nxql`

1. Unzip the `.apkg`.
2. Open `collection.anki2` (or `collection.anki21` if present) with the
   NexusQL SQLite-compat layer.
3. Parse `col.models` (JSON) to map Anki note types → canonical card types
   (`basic`, `reversed`, `cloze`, or `image_occlusion`).
4. For each note × template, emit one canonical card by applying the
   template's `qfmt`/`afmt` to the note's `flds` (0x1f-separated).
5. Walk `cards` table and translate scheduling to `card_srs` (table below).
6. Walk `revlog` into `reviews`.
7. Rehydrate media: Anki stores media under numeric filenames with a `media`
   JSON map; translate to content-addressed SHA-256 names.
8. Write `nexusql/`, `cards/*.json`, `compat/anki.sqlite` (pass-through copy
   of the original for perfect fidelity), and `manifest.json`.

#### Scheduling translation

| Anki field | Canonical | Transform |
|-|-|-|
| `cards.type` (0–3) | `srs.state` | 0→new, 1→learning, 2→review, 3→relearning |
| `cards.queue` (-3..4) | `srs.state` override | -1→suspended, -2/-3→buried |
| `cards.due` | `srs.next_due` | new: preserved as sort key only. learning: treat as unix-seconds → ms. review: `(col.crt + due*86400)*1000` |
| `cards.ivl` | `srs.interval` | positive days → seconds (`*86400`). negative already seconds. |
| `cards.factor` (permille) | `srs.ease` | `factor / 1000.0` |
| `cards.reps`, `.lapses` | `reps`, `lapses` | 1:1 |

#### Losses `.apkg → .nxql`

1. **Filtered decks** (`cards.odue`, `cards.odid`) — cards re-home to their
   original deck.
2. **Graves** (deletion tombstones) — meaningless in a portable file; dropped.
3. **Sync USN** (`col.usn`, `notes.usn`, `cards.usn`) — sync state is not portable.
4. **`cards.left`** (learning steps remaining today) — recomputed on next
   session. FSRS does not consume it.
5. **Custom scheduler JavaScript** (`col.conf.customSchedulerCode`) — dropped.
6. **Per-note-type CSS** is preserved in `compat/anki.sqlite` but only
   best-effort mapped onto canonical cards (the template is rendered once and
   the CSS discarded from the canonical view).

Because the original `.anki2` can be retained verbatim as `compat/anki.sqlite`,
a subsequent `.nxql → .apkg` round-trip is byte-lossless for users who never
modify the file in Shiori — they get back their original Anki collection.

### 5.2 `.nxql → .apkg`

1. Read canonical cards from `nexusql/`.
2. If `compat/anki.sqlite` exists **and the file was not modified** since
   creation (compare `nexusql/MANIFEST` mtime against `compat/anki.sqlite`
   mtime in ZIP metadata), zip `compat/anki.sqlite` + `media/*` and emit.
   This is the lossless path.
3. Otherwise, synthesize a fresh `collection.anki2`:
   - Translate canonical cards back to Anki notes + cards via a single
     synthetic note type per canonical `type`.
   - Re-encode media with numeric filenames and a `media` JSON map.
   - Translate scheduling state in reverse using the table in §5.1 (days
     rather than seconds, permille for ease).
4. Emit `collection.anki2`, media files, and `media` JSON into a ZIP named
   `*.apkg`.

#### Losses `.nxql → .apkg`

1. **FSRS-v5 parameters** beyond what Anki 23.10+ understands — downgraded to
   SM-2, with the FSRS state preserved in a `nxql.fsrs` tag for reverse
   mapping.
2. **Vector embeddings** and **`vectors.hnsw`** — no Anki equivalent.
3. **UUIDv7 card IDs** are remapped to Anki's epoch-ms IDs; originals stored
   in a `nxql.original_id` tag.
4. **Multiple canonical types sharing one note** — Anki requires a concrete
   note type; a distinct note type is generated per canonical type.

### 5.3 `.csv / .tsv → .nxql`

CSV is treated as **one deck, one card type, no media, no schedule** unless
reserved columns appear.

1. First row is the header; column names become card fields. The first two
   non-reserved columns become `front` and `back`.
2. Reserved columns (case-insensitive): `id`, `deck`, `tags`, `type`, `hint`,
   `extra`, `state`, `next_due`, `interval`, `ease`, `reps`, `lapses`,
   `last_reviewed`.
3. Each row → one canonical card. `id` defaults to UUIDv7 if not provided.
   Stability uses `sha256(canonicalized_row)[:16]` as a secondary dedup key.
4. Tags are split on whitespace or `;` (configurable via an import option).

TSV is identical except the delimiter is `\t` (matches Anki's native CSV
export).

#### Losses `csv/tsv → nxql`

1. Anki's cloze syntax `{{c1::text}}` is preserved verbatim in `front`; the
   canonical `type` defaults to `basic` unless `--type cloze` is passed or a
   `type` column is present.
2. Media is not inferred from path-looking cells.
3. No review history.

### 5.4 `.nxql → .csv / .tsv`

1. Single-type assumption: one CSV per canonical `type`. Multi-type files
   emit multiple CSVs plus a `bundle.json` pointing to them.
2. Columns in fixed order: `id, deck, type, front, back, hint, extra, tags,
   audio_ref, state, next_due, interval, ease, reps, lapses, last_reviewed`.
3. Tags joined with space.
4. Media becomes SHA-256 references; the bytes do NOT travel with CSV.

#### Losses `nxql → csv`

All binary data (media, vectors, HNSW), HTML formatting that breaks CSV
escaping, review history (unless explicitly exported as a second
`reviews.csv`).

### 5.5 `.xlsx ↔ .nxql`

One workbook ↔ one `.nxql`. Hidden sheets (leading `_`) carry metadata;
visible sheets carry cards.

| Sheet | Role |
|-|-|
| `_manifest` | Key/value pairs for `manifest.json` top-level fields. |
| `_decks` | Columns: `path`, `card_count`. Each visible sheet is one deck. |
| `<DeckName>` | One row per card. Columns match the canonical CSV schema. |
| `_reviews` | Optional review history. |
| `_media` | Optional. `name`, `sha256`, `mime`, `bytes_base64` inline. |

#### Losses `xlsx ↔ nxql`

- Cell styles, formulas, and charts are discarded.
- Media over 2 MB each is replaced with a relative path reference and bundled
  in a sidecar `media/` folder next to the workbook.
- HTML markup in fields is kept as raw text (Excel renders it as text, not
  rich content).

---

## 6. Caching strategy

**The NexusQL engine IS the cache.** There is no second cache layer above it.

### 6.1 Open-file flow

1. Reader computes `sha256` of the `.nxql` file.
2. If `~/.cache/nexusql/nxql/<file_id>/` exists and its `.meta.json`
   references the same `(source_path, source_sha256)`, **reuse the cache**
   (hot path — zero decompression).
3. Otherwise, extract the archive into the cache directory. Because
   `nexusql/**` and `vectors.hnsw` are stored uncompressed, extraction is
   just copy + fsync, not decompression.
4. Open `nexusql/` via the NexusQL engine. The engine's own LSM cache, block
   cache, and WAL replay handle all access-pattern optimization from here.
5. If `vectors.hnsw` exists, memory-map it and register with the HNSW index
   pointer in `CATALOG.json`. If absent, semantic queries scan
   `cards.vector`; on first such query, the engine lazily builds an index
   and persists it back into the cache so subsequent opens are warm.
6. If the caller only needs SQL compat (e.g. a non-NexusQL tool that
   understands SQLite), it can open `compat/anki.sqlite` directly with zero
   NexusQL involvement.

### 6.2 Shiori usage (zero-copy)

Shiori opens `.nxql` files directly via the NexusQL engine — there is no
intermediate decode step. The LSM + WAL handles hot-card residency
automatically. Typical cold-open target: **< 100 ms** for a 10k-card deck on
mid-range Android.

### 6.3 Read-only vs. read-write

- Default open mode is **read-only**. A session-scoped WAL overlay
  (`nexusql/WAL/session-<uuid>.wal`) captures mutations.
- `nxql.save(path)` folds the overlay into the main LSM, regenerates
  `cards/*.json`, `compat/anki.sqlite`, `compat/export.csv`,
  `compat/export.tsv`, and `vectors.hnsw`, recomputes `checksums`, and writes
  a new archive atomically via temp-file-rename.

### 6.4 Eviction

LRU when total nxql cache exceeds **2 GB** or individual entries are older
than **30 days** and have not been touched. Never evict an entry whose source
file still exists at its registered path.

---

## 7. Versioning and migration

### 7.1 Semver

- `manifest.format_version` — semver of the `.nxql` format itself.
- `manifest.nexusql_min_version` — minimum engine version to open natively.
- Per-vendor schemas under `extensions/<vendor>/` have their own versioning
  and do not affect format semver.

### 7.2 Compatibility matrix

| Reader vs file | Behavior |
|-|-|
| Reader major == file major | Open natively. |
| Reader major > file major | Auto-migrate (§7.3); emit `.bak` of original. |
| Reader major < file major | **Fallback path**: open `cards/*.json` (if present) and `compat/anki.sqlite` read-only. Advise user to upgrade. |
| Field added to a canonical type (minor bump) | Old readers see `null`; new readers see the field. |
| Field removed (major bump) | Writer MUST retain the old column in `cards/*.json` under `_deprecated__<name>` for one major version. |

The older-reader fallback is the explicit justification for carrying the
`cards/` mirror and the `compat/` exports — a file created by a future
NexusQL version can still be partially read by a past reader because the
canonical JSON and SQLite mirrors are written in frozen-schema form.

### 7.3 Migration mechanism

Migrations live under `src/nxql/migrations/v<from>_to_v<to>.rs`. Each one:

1. Opens the old archive read-only.
2. Produces a new archive in a temp directory.
3. Validates the new archive against the current schema.
4. Writes `<file>.bak` next to the original (kept 7 days, then pruned).
5. Atomically renames the new archive over the original.

Chained migrations are supported: opening a v1.0 file with a v3.0 reader
runs `v1_to_v2` then `v2_to_v3` in sequence.

`nexusql migrate <file.nxql>` CLI wraps this and prints a diff.

### 7.4 Deprecation policy

- Format-level deprecations announced one minor version before removal.
- Removed fields listed in `CHANGELOG.md` under `nxql-format`.
- Migrations for a given major are maintained for **at least 18 months**
  after the next major ships.

---

## 8. Security model

- **No code execution**. Template rendering is strictly mustache-style
  substitution. No JS, no `<script>` escape, no remote asset loading.
- **MIME sniffing + magic-byte validation** on every media file. A mismatch
  between declared mime and actual bytes fails the load.
- **Zip-slip protection**: any extracted path that resolves outside the cache
  directory is rejected.
- **Signature verification** (when `manifest.signature` is present) happens
  before any file is extracted. Failed signatures hard-error unless the
  caller passes `--unverified`.
- **Size limits**: 500 MB per entry, 5 GB total uncompressed. Enforced before
  decompression by reading the ZIP central directory.
- **Vector payloads** are capped at 16384 dims per card to prevent
  adversarial files from exhausting memory during HNSW build.

---

## 9. Implementation roadmap

### Phase A — Read-only opener

- Parse `manifest.json`, validate against the JSON schema.
- Open `nexusql/` via the existing NexusQL engine (read-only).
- Expose a Rust API: `NxqlFile::open(path) -> Result<NxqlFile>` with
  `.decks()`, `.cards(deck?)`, `.due_cards(at_ms)`, `.media(sha)`.
- JNI + Node bindings mirroring the Rust surface.
- UI milestone: Shiori displays cards and decks from a `.nxql` file on disk.
- **Deliverables**: `src/nxql/reader.rs`, `bindings/android/nxql.rs`,
  `bindings/node/nxql.ts`, fixture-based tests.

### Phase B — Write + save

- Serialize a live NexusQL catalog into the archive layout.
- Write `cards/*.json` and `cards/index.json` alongside the LSM.
- Content-address and dedupe media.
- Emit `manifest.checksums`; optional ed25519 signing.
- `nexusql pack <dir> -o <file.nxql>` CLI.
- **Deliverables**: `src/nxql/writer.rs`, round-trip tests (read → write →
  read produces byte-identical output when nothing was mutated).

### Phase C — `.apkg` import/export

- Importer: unzip → parse `collection.anki2` → translate schema.
- Exporter: pass-through of `compat/anki.sqlite` when untouched; synthesis
  from canonical when modified. Respect "Support older Anki versions" flag.
- Per-converter lossiness report emitted to
  `extensions/nxql/conversion-report.json`.
- **Deliverables**: `src/nxql/convert/apkg.rs`, end-to-end round-trip fixtures
  (apkg → nxql → apkg preserves all non-lossy fields).

### Phase D — CSV / TSV / XLSX import/export

- `src/nxql/convert/csv.rs` with explicit delimiter flag.
- `src/nxql/convert/xlsx.rs` using `calamine` (read) + `rust_xlsxwriter` (write).
- A `--no-compat` writer flag that skips `compat/` to reduce file size when
  the consumer is NexusQL-native.
- **Deliverables**: three converters plus unified lossiness reporting.

### Phase E — Vector search integration

- Persist the HNSW graph to `vectors.hnsw` in a stable, documented on-disk
  layout (separate doc: `docs/hnsw-ondisk.md`).
- Memory-map on open; fall back to linear scan when absent.
- Lazy-build indexes on first semantic query for files that shipped without
  them; persist back into the cache.
- `nexusql nxql reindex <file>` CLI to force a rebuild.
- **Deliverables**: `src/nxql/indexes/hnsw.rs`, benchmarks demonstrating
  cold-open < 100 ms and warm semantic search < 10 ms at 10k cards.

---

## 10. Open questions

1. **Schedule on import — reset or preserve?** Currently the spec preserves
   schedule state whenever present. A `?mode=reset` hint on import would
   let "use these cards as a fresh deck" flows skip the `card_srs` table.
2. **Is `compat/anki.sqlite` always worth the size?** For a 10k-card Japanese
   deck with media, it adds ~5 MB. Proposal: `--no-compat` writer flag;
   default generates compat for interop.
3. **Cloze deletion fidelity.** Anki's cloze syntax (`{{c1::text::hint}}`) is
   stored verbatim in `front`. Rendering in Shiori requires implementing the
   cloze transformer. Do we ship a canonical cloze renderer in the reader?
4. **Image occlusion.** Anki's add-on produces SVG-masked images. The
   canonical `type: "image_occlusion"` carries a JSON sidecar with mask
   coordinates. Concrete shape is TBD pending a field sample.
5. **Provenance chain growth.** Each import extends `manifest.provenance[]`.
   Long-running files could accumulate hundreds of entries. Proposal: keep
   most-recent 5; rotate older entries into
   `extensions/nxql/provenance.history.json`.

---

## 11. References

- Anki `.apkg` internals: Anki-Android wiki "Database Structure",
  brandur.org "Anki's .apkg".
- FSRS-v5 algorithm: `open-spaced-repetition/fsrs4anki` specification.
- NexusQL engine: `README.md`, `docs/architecture/neural-technologies.md`.
- UUIDv7: RFC 9562 §5.7.
- Apache Parquet 2.9 (referenced only for future `_reviews` alt-form; not
  used in v1.0 archive layout).
