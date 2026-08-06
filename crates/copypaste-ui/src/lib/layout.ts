/**
 * INV-5: row heights are over-reserved, never estimated from the content. The
 * rendered height depends on pane width, which a pure height function cannot
 * know; the "smarter" character-count estimate under-reserved at narrow widths
 * and overlapped rows site-wide (CopyPaste-g27b.30).
 *
 * The pixel values below are derived from design tokens. If `--pad-row-y` or
 * `--lh-normal` changes, these change with it or rows overlap.
 */

/** Row vertical padding, both halves. 2 × `--pad-row-y` (10px). */
export const ROW_PAD_V = 20;

/** `--fs-sm` 14px × `--lh-normal` 1.5. */
export const TITLE_LINE_PX = 21;

/** `--fs-xs` 12px × `--lh-normal` 1.5, and the gap above it. */
export const META_LINE_PX = 18;
export const META_MARGIN_PX = 8;

/** 21 + 8 + 18 + 20 = 67. Guarantees a strictly positive gap to the next row. */
export const SINGLE_LINE_FLOOR =
  TITLE_LINE_PX + META_MARGIN_PX + META_LINE_PX + ROW_PAD_V;


export const MIN_PREVIEW_LINES = 1;
export const MAX_PREVIEW_LINES = 6;
export const DEFAULT_PREVIEW_LINES = 2;

/** Images use the same row composition as text, but can use more vertical
 * room so a copied screenshot remains recognizable. */
// 3.75 is 1.5× the previous 2.5× cap. Keep this single shared cap so History
// and Quick Paste reserve identical virtual space on every platform.
export const IMAGE_PREVIEW_HEIGHT_MULTIPLIER = 3.75;

/** A function of the *setting*, never of the item (INV-5). Lowering it shrinks
 *  total height below the scroll offset, which is what INV-6's clamp is for. */
export function rowHeight(previewLines: number): number {
  const lines = Math.min(
    MAX_PREVIEW_LINES,
    Math.max(MIN_PREVIEW_LINES, Math.round(previewLines)),
  );
  return SINGLE_LINE_FLOOR + (lines - 1) * TITLE_LINE_PX;
}

/**
 * The preview itself occupies the same vertical space as the configured text
 * lines. Metadata keeps its own reserved line below it, so image rows and text
 * rows always fit the same virtualized height.
 */
export function rowPreviewHeight(previewLines: number): number {
  const lines = Math.min(
    MAX_PREVIEW_LINES,
    Math.max(MIN_PREVIEW_LINES, Math.round(previewLines)),
  );
  return lines * TITLE_LINE_PX;
}

/** The maximum rendered height of an image preview for a given line setting. */
export function imagePreviewHeight(previewLines: number): number {
  return rowPreviewHeight(previewLines) * IMAGE_PREVIEW_HEIGHT_MULTIPLIER;
}

/**
 * Image rows reserve the image cap, metadata, and the same vertical padding as
 * every other row. This keeps virtualized offsets correct without measuring
 * image content at runtime.
 */
export function imageRowHeight(previewLines: number): number {
  return imagePreviewHeight(previewLines) + META_MARGIN_PX + META_LINE_PX + ROW_PAD_V;
}

/** §5.2: 240px above and below. TanStack counts rows, so convert. */
export const OVERSCAN_PX = 240;
export function overscanRows(previewLines: number): number {
  return Math.ceil(OVERSCAN_PX / rowHeight(previewLines));
}

/** The daemon clamps with its own maximum. */
export const PAGE_SIZE = 200;
/** Fire load-more within 300px of the bottom. */
export const LOAD_MORE_THRESHOLD_PX = 300;
export const SEARCH_LIMIT = 500;
export const SEARCH_DEBOUNCE_MS = 250;
/** Shorter than the FTS debounce: the client filter is what the user watches
 *  while typing, so it may not wait a quarter of a second. */
export const FILTER_DEBOUNCE_MS = 60;
export const UNDO_WINDOW_MS = 5000;
export const COPY_FLASH_MS = 700;
export const REVEAL_TIMEOUT_MS = 10_000;
/** Row icons and thumbnails survive the virtualizer recycling a row, but not
 *  indefinitely: a thumbnail must not outlive the clipping it was made from. */
export const MEDIA_GC_MS = 300_000;
export const ICON_STALE_MS = 300_000;

/** §5.1: slowed from 1200ms — 50→20 IPC calls/min with no perceptible
 *  difference (s7ia B1). The backoff exists so a dead service is not hammered. */
export const POLL_ACTIVE_MS = 3000;
export const POLL_BACKOFF_MS = 5000;
/** What the poll slows to while the change stream is delivering. Not zero —
 *  `useHistory.pollInterval` says why. */
export const POLL_PUSH_BACKSTOP_MS = 30_000;
/** Faster than the list: a 10s poll left the chip green long after the service
  *  had died (CopyPaste-f701). */
export const STATUS_POLL_MS = 2000;
/** Decoupled from the 2s status poll, which used to drag `peers` along at
 *  30 calls/min (CopyPaste-crh3.48). */
export const PEERS_POLL_MS = 10_000;
