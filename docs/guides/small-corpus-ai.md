# Short-string search and one-shot classification

## Lightweight ColBERT scoring

```sql
SELECT name, COLBERT_SCORE('EECM-021', name) AS relevance
FROM devices ORDER BY relevance DESC LIMIT 10;
```

`COLBERT_SCORE(query, candidate [, 'lightweight'])` is available in the ordinary
SQL engine. It does not download or load a model and also builds with
`--no-default-features`.

The mode embeds Unicode character bigrams and trigrams into deterministic,
normalized 64-dimensional token vectors, then uses the existing ColBERT MaxSim
operator. Scores are normalized to [0,1]. Ninety percent of the score ignores
punctuation; ten percent retains punctuation, so exact `EECM-021` ranks above
`EECM021`, which ranks above `EECM-210`. Case is ignored. Japanese characters are
retained. ORDER BY evaluates each score once per source row.

This measures lexical resemblance for identifiers and short labels. It is not
a learned semantic model and does not translate between Japanese and English.
Inputs are limited to 256 Unicode characters each; longer or invalid inputs
return NULL. Empty strings score 0. Scores are relevance values, not probabilities.

## One-shot HDC classifier

```sql
CREATE TABLE classifier (model BLOB);
INSERT INTO classifier VALUES
    (HDC_LEARN(NULL, 'ZONE-A', 'SW-ALPHA', 'Gi1/0/1', 101));
UPDATE classifier SET model =
    HDC_LEARN(model, 'ZONE-B', 'SW-BETA', 'Gi1/0/1', 202);

SELECT HDC_PREDICT(model, 'SW-ALPHA', 'Gi1/0/17', 101) AS zone,
       HDC_CONFIDENCE(model, 'SW-ALPHA', 'Gi1/0/17', 101) AS confidence
FROM classifier;
```

One example per label is sufficient. The unseen port is classified using its
shared switch, VLAN, and port-name structure. Feature positions must have the
same meaning during training and prediction.

| Function | Behavior |
| --- | --- |
| `HDC_LEARN(model_or_null, label, feature1, ...)` | Returns a new model BLOB; NULL starts an empty model |
| `HDC_MERGE(model1, model2, ...)` | Combines models with the same feature count |
| `HDC_PREDICT(model, feature1, ...)` | Returns the best label |
| `HDC_CONFIDENCE(model, feature1, ...)` | Returns the non-negative similarity margin over the runner-up |

Learning is a pure scalar function: save its result in a column using INSERT or
UPDATE. Existing labels are replaced by the new single example. Merging uses
the later argument's prototype when labels overlap. For one-shot training from
existing rows, add a BLOB column and evaluate the fields directly:

```sql
-- examples(zone, switch_name, port, vlan, model) has one row per zone
UPDATE examples SET model = HDC_LEARN(NULL, zone, switch_name, port, vlan);
SELECT HDC_MERGE(a.model, b.model) AS combined_model
FROM examples a JOIN examples b ON a.zone != b.zone
WHERE a.zone = 'ZONE-A' AND b.zone = 'ZONE-B';
```

Models use deterministic 4,096-dimensional MAP hypervectors. Text n-grams are
bundled into field vectors, bound to positional role vectors, and bundled into
the row prototype. Numeric and Boolean features are categorical, not continuous
measurements. Predictions compare cosine similarity to every stored prototype;
ties pick the lexical-first label with confidence zero.

Confidence is in [0,1] and is **not a calibrated probability**. With only one label,
the runner-up similarity is zero. An unfamiliar row can still receive a label;
use held-out examples to choose a confidence cutoff suitable for the application.

The BLOB includes a format version, feature count, labels, and packed prototypes.
It can be copied, bound as a parameter, and persisted like any other BLOB. There
is no global model registry. In-memory transaction snapshots restore model values;
disk transaction durability follows the existing core engine (which currently
writes DML before COMMIT).

Limits: 1–32 scalar features, 256 labels, 256 Unicode characters per text feature
or label, and 1 MiB per serialized model. NULL features, malformed models,
unsupported feature types, and feature-count mismatches return NULL, following
the scalar evaluator's convention. Both lightweight scoring and classification
work with `cargo test --no-default-features`; neither loads a model file.
