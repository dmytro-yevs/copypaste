# Clipboard search

History, Quick Paste, and the search API share one Rust search compiler over
SQLite FTS5. It searches the complete retained text history before applying
filters, sorting, and pagination. It does not use models, embeddings, remote
services, or additional production dependencies.

## Matching

- Every unquoted word is a prefix; words can appear in any order.
- Words of 4–64 normalized characters also match infixes and spelling errors.
  Alphabetic words of 4–7 characters allow one edit; longer words allow two.
  An edit is an insertion, deletion, substitution, or adjacent transposition.
  Spelling matching also works on an unfinished prefix.
- Short words and numeric identifiers retain prefix behavior without spelling
  expansion. Numeric fragments of at least four characters can match infixes.
- All query words must match. This prevents a single common word from flooding
  a multiword query with unrelated results.
- Quoted phrases remain exact and adjacent. A star after the closing quote
  allows the final word to be a prefix, as before.
- SQLite's unicode61 tokenizer handles normalization, including case and its
  supported diacritic removal. No language-specific synonym rules are applied.

For example, `meting notse`, `notes meetign`, and `board` can find
`meeting notes clipboard`. `"meting notes"` does not find `meeting notes`.

## Ranking and resource bounds

Relevance sorting puts original prefix/phrase matches before spelling/infix
matches, with BM25 ranking within each tier. Existing pinned-item order takes
precedence in History and Quick Paste. History starts a new search with Best
match and preserves a different sort selected during the search.

Expansion streams distinct words from the existing FTS vocabulary rather than
loading or decrypting clip bodies. It examines vocabulary words of up to 66
characters and retains at most 16 alternatives for each of the first 16
eligible query words, ordered by edit cost, document frequency, and word.
Every original query term remains searchable regardless of those bounds.
Memory for spelling comparison and retained alternatives is bounded. Vocabulary
scan time grows with the number of distinct indexed words; it is not a constant
time or approximate-nearest-neighbor index.

Temporary per-connection FTS tables normalize the query using the same tokenizer;
their input rows are cleared after normalization. No second persistent index or
database migration is needed. Inserts, sync, deletion, retention, restore, and
reopening automatically use the current vocabulary. Relevance queries sort ids
and scores before fetching payloads, retaining the existing page and byte budgets.

Semantic similarity is provided separately by the optional
[`Semantic Search module`](../modules/semantic-search/README.md). The shared host
merges its live, hash-bound candidates before filters and cursor paging. The
built-in fuzzy compiler continues to have no model loading or inference code.
Model assets are downloaded only after language selection, and its vector index
is local derived data inside the encrypted history database.

## Verification

```sh
cargo test -p copypaste-core storage::
cargo bench -p copypaste-core --bench history -- 'history/search'
cd apps/copypaste_flutter
flutter test test/features/history/history_controller_test.dart
```

The benchmark uses encrypted file-backed databases with 1,000 and 10,000 clips,
including prefix, typo, infix, multiword, no-match, and ranked-page queries.
An additional 10,000-clip fixture adds 80,000 distinct alphabetic vocabulary
words to expose the cost of vocabulary expansion on less repetitive content.
Host checks do not replace native qualification on Android or Windows.

A short local optimized run on macOS ARM64 on 2026-10-08 measured:

| Fixture | Query | Estimated time |
| --- | --- | --- |
| 10,000 paragraph clips | `quick`, first 500 results | 20 ms |
| 10,000 paragraph clips | `quik`, first 50 results | 80 ms |
| 10,000 paragraph clips | `quik brwon fox`, first 50 results | 103 ms |
| 10,000 paragraph clips | Same query, first ranked History page | 151 ms |
| 10,000 clips plus 80,000 distinct words | `topicaaab`, first 50 results | 201 ms |
| 10,000 clips plus 80,000 distinct words | `quik brwon fox`, first 50 results | 226 ms |

This used 10 samples, a 200 ms warmup, and a 500 ms requested measurement
window. Concurrent native builds made timings noisy. These synthetic host
measurements describe this run, not latency guarantees or mobile measurements.
