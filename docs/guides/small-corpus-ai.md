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
