/**
 * One ranking rule for every search box in the app. History, Quick Paste and
 * Settings each grew their own — two copies of the same fuzzysort pipeline and
 * a third that matched substrings — so the same query ranked three ways.
 *
 * Ties break on the caller's original order (INV-2): an idle poll that returned
 * the same rows must not reshuffle them.
 */
import fuzzysort from "fuzzysort";

/** A `Prepared` target is indexed once; see `fuzzyTargets` for why that matters
 *  at 200 items. `null` marks a candidate that must never match — a sensitive
 *  item's absent plaintext, not a miss. */
export type FuzzyTarget = Fuzzysort.Prepared | string | null | undefined;

function bestScore(query: string, targets: readonly FuzzyTarget[]): number | null {
  let best: number | null = null;
  for (const target of targets) {
    if (target === null || target === undefined) continue;
    const match = fuzzysort.single(query, target);
    if (match !== null && (best === null || match.score > best)) best = match.score;
  }
  return best;
}

/**
 * The best-scoring candidate decides an item's rank, so a field-per-candidate
 * caller gets what `fuzzysort.go`'s `keys` would give it without every call
 * site restating the options.
 *
 * An empty query returns the array it was given, unchanged and by reference.
 */
export function rankFuzzy<T>(
  items: readonly T[],
  query: string,
  targetsOf: (item: T) => readonly FuzzyTarget[],
): readonly T[] {
  const needle = query.trim();
  if (needle.length === 0) return items;

  return items
    .map((item, index) => ({ item, index, score: bestScore(needle, targetsOf(item)) }))
    .filter((entry): entry is typeof entry & { score: number } => entry.score !== null)
    .sort((a, b) => b.score - a.score || a.index - b.index)
    .map((entry) => entry.item);
}
