/**
 * Pure virtualizer math for HistoryView's VirtualList.
 *
 * Renders only the rows intersecting the viewport plus an overscan buffer.
 * Row heights are computed from rowHeightFor (supporting mixed image/text
 * heights), stored in a prefix-sum table, and binary-searched for the first
 * visible row — O(log n) per scroll event.
 */
import { isImageType, type HistoryEntry } from "../../lib/ipc";

export const OVERSCAN_PX = 240; // render a buffer above/below the viewport

// g27b.30: `.row`'s own vertical padding (tokens.css --pad-row: 9px 10px —
// 9px top + 9px bottom) is CONSTANT across every row kind and every density
// value (patterns.css's `.row` rule does not vary by density at all — the
// density axis itself was frozen to "comfortable" in the Phase 4 redesign,
// see HistoryView.tsx's `const density = "comfortable" as const`). Every
// height computed below must reserve at least this much on top of the row's
// actual content, or `.row`'s `max-height` (fed straight from this file's
// return value via HistoryRow's `--row-max`) clips shorter than the real
// rendered box and the overflow bleeds into the next row.
const ROW_PAD_V = 18;
// Single title line's rendered height: --fs-base (14px) * the inherited
// --lh-normal line-height (1.5, set globally on body in base.css) = 21px.
const TITLE_LINE_PX = 21;
// .row__title(21) + .row__meta's margin-top (--s-1, 2px) + one meta line
// (--fs-sm 11.5px * 1.5 ≈ 17.25px, rounded up to 18 so the floor always
// leaves a strictly POSITIVE gap to the next row instead of an exact-fit 0)
// + the row's own vertical padding (ROW_PAD_V) = 59px.
const SINGLE_LINE_FLOOR = TITLE_LINE_PX + 2 + 18 + ROW_PAD_V;

/**
 * Compute the row height (px) for an entry.
 *
 * §2 / §5 density rules (historical intent — the density axis is currently
 * frozen to "comfortable" in production, so these numbers are floors rather
 * than the sole source of truth; see SINGLE_LINE_FLOOR / ROW_PAD_V above):
 *  - Text rows: 42px (spacious), 34px (comfortable) or 28px (compact), floor at 22px.
 *  - Image rows: `imageMaxHeight` + 20px (spacious), +12px (comfortable) or +8px (compact), min 34px.
 *  - File rows: fixed 44px (fits FileChip regardless of density).
 *
 * Kept in one place so the virtualizer's prefix-sum offset math stays in sync
 * with what HistoryRow actually renders.
 */
export function rowHeightFor(
  entry: HistoryEntry,
  previewSize: number,
  imageMaxHeight: number,
  density: "comfortable" | "compact" | "spacious" = "comfortable",
  // Preview-lines setting: text rows grow by one line-height per extra line so
  // the virtualizer's allocated height matches HistoryRow's multi-line clamp.
  previewLines = 1
): number {
  const isImage = isImageType(entry.content_type);
  // File rows get a fixed height that fits the FileChip (icon + filename + buttons).
  const isFile = entry.content_type === "file";
  if (isImage) {
    // §2: image padding 20px spacious, 12px comfortable, 8px compact. g27b.25:
    // the rendered thumbnail is CSS-capped at exactly `imageMaxHeight` (see
    // HistoryRow's `--img-max` var / .tile--thumb img max-height), so the
    // worst case is imageMaxHeight + the row's real vertical padding
    // (ROW_PAD_V) — the old comfortable/compact pads (12/8) under-shot that
    // by 6/10px. Math.max keeps the historical numbers wherever they already
    // clear the real floor (spacious already did).
    const pad = Math.max(
      ROW_PAD_V,
      density === "spacious" ? 20 : density === "compact" ? 8 : 12
    );
    return Math.max(imageMaxHeight + pad, 34);
  }
  if (isFile) return 44; // FileChip is taller than a single-line text row
  // §2: spacious = 42px, comfortable = 34px, compact = 28px (floor at 22px).
  // g27b.30: Math.max reconciles these against SINGLE_LINE_FLOOR — the real
  // box every text row renders regardless of density (see comment above) —
  // so none of the three ever allocates less than the content actually needs.
  const base = Math.max(
    SINGLE_LINE_FLOOR,
    density === "spacious" ? 42 : density === "compact" ? 28 : 34
  );
  const single = Math.max(previewSize, base, 22);
  if (previewLines <= 1) return single;
  // previewLines > 1: HistoryRow's inline titleStyle switches the title to a
  // `-webkit-line-clamp: previewLines` box, which can render up to
  // `previewLines` lines. How many lines a given clip actually wraps to
  // depends on the row's rendered WIDTH, which this pure function (used to
  // build the virtualizer's offset table before layout) cannot know. The
  // previous heuristic estimated wrapped lines from a width-agnostic
  // char-count (~120 chars/line — true only at wide desktop widths); at the
  // narrow end of the supported range (400px) real lines fit far fewer
  // characters, so a clip estimated at "2 lines" could really wrap to 6+ and
  // overflow the allocated height into the next row (site-wide overlap in
  // the g27b.30 audit). Always reserving the full `previewLines` cap is the
  // only width-independent way to guarantee the row never overflows it —
  // the clamp itself hard-caps the rendered title at exactly that many
  // lines no matter how long the text is, so this is never an under-reserve.
  return single + (previewLines - 1) * TITLE_LINE_PX;
}

/**
 * Build the prefix-sum offset table for a list of row heights.
 * `offsets[i]` is the top edge (px) of row `i`; `offsets[n]` is total height.
 * Exported for unit testing the virtualization math.
 */
export function buildOffsets(heights: number[]): number[] {
  const arr = new Array<number>(heights.length + 1);
  arr[0] = 0;
  for (let i = 0; i < heights.length; i++) arr[i + 1] = arr[i] + heights[i];
  return arr;
}

/**
 * Given a prefix-sum offset table, the scroll position, and the viewport
 * height, return the `[start, end)` index range of rows to render (inclusive
 * of an overscan buffer). Pure and side-effect free. `end` is exclusive.
 */
export function computeVisibleWindow(
  offsets: number[],
  scrollTop: number,
  viewportH: number,
  overscanPx: number = OVERSCAN_PX
): { start: number; end: number } {
  const count = offsets.length - 1;
  if (count <= 0) return { start: 0, end: 0 };

  const top = Math.max(0, scrollTop - overscanPx);
  const bottom = scrollTop + viewportH + overscanPx;

  // Binary-search the first row whose bottom edge is past `top`.
  let lo = 0;
  let hi = count;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (offsets[mid + 1] <= top) lo = mid + 1;
    else hi = mid;
  }
  const start = Math.min(lo, count - 1);

  let end = start;
  while (end < count && offsets[end] < bottom) end++;
  return { start, end };
}
