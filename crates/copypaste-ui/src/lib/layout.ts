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
export const META_MARGIN_PX = 4;

/** 21 + 4 + 18 + 20 = 63. Guarantees a strictly positive gap to the next row. */
export const SINGLE_LINE_FLOOR =
  TITLE_LINE_PX + META_MARGIN_PX + META_LINE_PX + ROW_PAD_V;


export const MIN_PREVIEW_LINES = 1;
export const MAX_PREVIEW_LINES = 6;
export const DEFAULT_PREVIEW_LINES = 2;

/** A function of the *setting*, never of the item (INV-5). Lowering it shrinks
 *  total height below the scroll offset, which is what INV-6's clamp is for. */
export function rowHeight(previewLines: number): number {
  const lines = Math.min(
    MAX_PREVIEW_LINES,
    Math.max(MIN_PREVIEW_LINES, Math.round(previewLines)),
  );
  return SINGLE_LINE_FLOOR + (lines - 1) * TITLE_LINE_PX;
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
export const UNDO_WINDOW_MS = 5000;
export const COPY_FLASH_MS = 700;
export const REVEAL_TIMEOUT_MS = 10_000;

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
