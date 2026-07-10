// ---------------------------------------------------------------------------
// Fuzzy matcher — dependency-free, subsequence-based (Maccy-style)
// ---------------------------------------------------------------------------
//
// Algorithm:
//   1. Subsequence check: every query character must appear in order in the
//      target (case-insensitive). This is the gate — no match → null.
//   2. Scoring rewards:
//      - Contiguous run bonus: consecutive matched chars score higher.
//      - Start-of-word/camelCase bonus: matched char immediately follows a
//        separator (space, -, _, /, \, .) OR follows a lowercase→uppercase
//        boundary (camelCase).
//      - Earliness bonus: matches closer to the start of the string score
//        higher, scaled by 1 / (1 + position).
//      - Full-prefix bonus: entire query matches as a literal prefix.
//   3. Returns matched character index positions for highlight rendering.
//
// The greedy left-to-right placement is good enough for a quick-access popup.
// We do NOT backtrack for optimal alignment (that would be O(n·m) dp per
// keystroke for potentially hundreds of items) — greedy is O(n+m) per item.

export interface FuzzyResult {
  /** Higher is better — use for sort (descending). */
  score: number;
  /** Indices (in the original `text`) of matched characters. */
  positions: number[];
}

const CONTIGUOUS_BONUS = 8;
const WORD_START_BONUS = 12;
const CAMEL_BONUS = 10;
const EARLINESS_SCALE = 3;
const PREFIX_BONUS = 20;

/** Returns null if query does not match target as a subsequence. */
export function fuzzyMatch(query: string, text: string): FuzzyResult | null {
  if (query.length === 0) return { score: 0, positions: [] };

  const q = query.toLowerCase();
  const t = text.toLowerCase();
  const tLen = t.length;
  const qLen = q.length;

  // CopyPaste-f72f: prefer a literal contiguous substring match over the
  // scattered subsequence match below. The plain greedy subsequence scan
  // picks the *earliest* occurrence of each query char independently, which
  // can highlight isolated letters spread across unrelated words even when
  // the query also occurs as one contiguous run elsewhere in the text (e.g.
  // "cat" against "concatenate" — greedy subsequence lights up "c-o-n-CAT-…"
  // scattered, when the literal "cat" substring is right there). When the
  // query is a literal substring, treat it as one contiguous highlighted
  // run — much easier to read — and let the earliness/prefix scoring below
  // still apply unchanged (match ordering is not affected: this only changes
  // which character *positions* get highlighted for an already-matching
  // item, not whether an item matches or how it's ranked among matches with
  // different substring/subsequence status, since ordering was already
  // subsequence-based and a substring is always also a valid subsequence).
  const substrIndex = t.indexOf(q);
  const positions: number[] = [];
  let qi = 0;
  if (substrIndex !== -1) {
    for (let i = 0; i < qLen; i++) positions.push(substrIndex + i);
    qi = qLen;
  } else {
    // Fast path: not even a subsequence.
    for (let ti = 0; ti < tLen && qi < qLen; ti++) {
      if (t[ti] === q[qi]) {
        positions.push(ti);
        qi++;
      }
    }
  }
  if (qi < qLen) return null; // not a full subsequence

  // Score the greedy placement.
  let score = 0;
  for (let i = 0; i < positions.length; i++) {
    const pos = positions[i]!;

    // Earliness: reward matches near the start.
    score += EARLINESS_SCALE / (1 + pos);

    // Contiguity: bonus when this match immediately follows the previous.
    if (i > 0 && positions[i - 1]! === pos - 1) {
      score += CONTIGUOUS_BONUS;
    }

    // Word-start: match right after a separator character.
    if (pos === 0) {
      score += WORD_START_BONUS;
    } else {
      const prev = text[pos - 1]!;
      if (" -_/\\.".includes(prev)) {
        score += WORD_START_BONUS;
      }
      // CamelCase boundary: previous char is lowercase, current is uppercase.
      const cur = text[pos]!;
      if (prev >= "a" && prev <= "z" && cur >= "A" && cur <= "Z") {
        score += CAMEL_BONUS;
      }
    }
  }

  // Full-prefix bonus: entire query is a contiguous prefix match.
  if (t.startsWith(q)) {
    score += PREFIX_BONUS;
  }

  return { score, positions };
}
