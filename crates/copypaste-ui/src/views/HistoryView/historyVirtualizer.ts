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

/**
 * Compute the row height (px) for an entry.
 *
 * §2 / §5 density rules:
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
  previewLines: number = 1
): number {
  const isImage = isImageType(entry.content_type);
  // File rows get a fixed height that fits the FileChip (icon + filename + buttons).
  const isFile = entry.content_type === "file";
  if (isImage) {
    // §2: image padding 20px spacious, 12px comfortable, 8px compact.
    const pad = density === "spacious" ? 20 : density === "compact" ? 8 : 12;
    return Math.max(imageMaxHeight + pad, 34);
  }
  if (isFile) return 44; // FileChip is taller than a single-line text row
  // §2: spacious = 42px, comfortable = 34px, compact = 28px (floor at 22px).
  const base = density === "spacious" ? 42 : density === "compact" ? 28 : 34;
  const single = Math.max(previewSize, base, 22);
  // Estimate how many lines THIS clip actually needs (explicit newlines +
  // rough width-agnostic wrap), capped at the previewLines setting — so short
  // clips stay compact (no dead gap) while long clips grow up to the limit.
  const LINE_PX = 20;
  const CHARS_PER_LINE = 120;
  const explicit = (entry.preview.match(/\n/g)?.length ?? 0) + 1;
  const wrapped = Math.ceil(entry.preview.length / CHARS_PER_LINE);
  const estLines = Math.min(previewLines, Math.max(1, explicit, wrapped));
  return single + Math.max(0, estLines - 1) * LINE_PX;
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
