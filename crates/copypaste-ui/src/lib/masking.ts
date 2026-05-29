// ---------------------------------------------------------------------------
// Sensitive-span masking
// ---------------------------------------------------------------------------

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
