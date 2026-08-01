export interface FuzzyResult {
  readonly score: number;
  readonly positions: readonly number[];
}

const CONTIGUOUS_BONUS = 8;
const WORD_START_BONUS = 12;
const CAMEL_BONUS = 10;
const EARLINESS_SCALE = 3;
const PREFIX_BONUS = 20;

/** The score and matched positions for a case-insensitive subsequence. */
export function fuzzyMatch(query: string, text: string): FuzzyResult | null {
  if (query.length === 0) return { score: 0, positions: [] };

  const needle = query.toLowerCase();
  const haystack = text.toLowerCase();
  // CopyPaste-f72f: keep a literal hit together instead of highlighting an
  // earlier scattering of the same characters.
  const substring = haystack.indexOf(needle);
  const positions: number[] = [];

  if (substring >= 0) {
    for (let index = 0; index < needle.length; index += 1) {
      positions.push(substring + index);
    }
  } else {
    let needleIndex = 0;
    for (let index = 0; index < haystack.length && needleIndex < needle.length; index += 1) {
      if (haystack[index] === needle[needleIndex]) {
        positions.push(index);
        needleIndex += 1;
      }
    }
    if (needleIndex < needle.length) return null;
  }

  let score = 0;
  for (let index = 0; index < positions.length; index += 1) {
    const position = positions[index]!;
    score += EARLINESS_SCALE / (1 + position);
    if (index > 0 && positions[index - 1] === position - 1) score += CONTIGUOUS_BONUS;

    if (position === 0) {
      score += WORD_START_BONUS;
      continue;
    }

    const previous = text[position - 1]!;
    const current = text[position]!;
    if (" -_/\\.".includes(previous)) score += WORD_START_BONUS;
    if (previous >= "a" && previous <= "z" && current >= "A" && current <= "Z") {
      score += CAMEL_BONUS;
    }
  }

  if (haystack.startsWith(needle)) score += PREFIX_BONUS;
  return { score, positions };
}
