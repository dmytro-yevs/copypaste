// ---------------------------------------------------------------------------
// Sensitive-span masking
// ---------------------------------------------------------------------------

/**
 * Returns true when a clipboard entry should be visually blurred/redacted.
 *
 * An entry is masked when:
 * - `maskSensitive` preference is enabled (user hasn't turned off masking), AND
 * - the entry is classified as fully sensitive (`is_sensitive === true`).
 *
 * Partial span masking (sensitive_spans only) does NOT trigger blur — those
 * spans are redacted via applySpanMasking in-place, which is already visible.
 */
export function shouldMask(
  entry: { is_sensitive: boolean },
  maskSensitive: boolean,
): boolean {
  return maskSensitive && entry.is_sensitive;
}

/**
 * Redact the sensitive ranges in `spans` from `text`, replacing each range with
 * bullet characters.
 *
 * `spans` are **Unicode scalar (code-point) offsets** — the same units the
 * daemon emits (it counts characters, not UTF-16 units). JavaScript strings are
 * UTF-16, so `String.prototype.slice` / `length` would mis-index any text that
 * contains astral characters (e.g. emoji), shifting the mask and potentially
 * revealing part of a secret. To stay correct we operate on an array of code
 * points (`Array.from`), slice/index that, and join back.
 *
 * Ranges are clamped to the code-point length of the text and processed
 * left-to-right; the mask length is the code-point count of each span.
 */
export function applySpanMasking(text: string, spans: Array<[number, number]>): string {
  if (spans.length === 0) return text;
  const chars = Array.from(text);
  const len = chars.length;
  let result = "";
  let cursor = 0;
  // Sort spans by start index so we process left-to-right.
  const sorted = [...spans].sort((a, b) => a[0] - b[0]);
  for (const [start, end] of sorted) {
    const s = Math.min(Math.max(start, cursor), len);
    const e = Math.min(end, len);
    if (s > cursor) result += chars.slice(cursor, s).join("");
    if (e > s) result += "•".repeat(e - s);
    cursor = Math.max(cursor, e);
  }
  result += chars.slice(cursor).join("");
  return result;
}
