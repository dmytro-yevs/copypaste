/**
 * Row geometry for the virtualiser, and the timing constants — manifest 06
 * §3.1.3, §5.1, §5.2, §5.3.
 *
 * INV-5: **row heights are over-reserved, never estimated from the content.**
 * The rendered height of a preview depends on the pane width, which a pure
 * height function cannot know, so every text row reserves the full preview-line
 * cap whether or not the clip fills it. The "smarter" character-count estimate
 * was itself the bug (CopyPaste-g27b.30): it under-reserved at narrow widths
 * and rows overlapped site-wide.
 *
 * The row renders inside a box of exactly this height with `overflow: hidden`,
 * so the reservation is an upper bound by construction rather than by
 * arithmetic.
 *
 * Every pixel below is derived from a design token, and the derivation is the
 * load-bearing part: if `--pad-row-y` or `--lh-normal` changes in
 * `design/tokens`, these numbers change with it or rows overlap.
 *
 * The density axis ("compact" / "comfortable" / "spacious") is not ported:
 * production froze it to "comfortable" and manifest §9.2 says collapse it.
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

/** The bounds of the `previewLines` preference (Settings › List). */
export const MIN_PREVIEW_LINES = 1;
export const MAX_PREVIEW_LINES = 6;
export const DEFAULT_PREVIEW_LINES = 2;

/**
 * The reserved height of a row at a given preview-line cap.
 *
 * Constant in the content by design: it is a function of the *setting*, never
 * of the item (INV-5). Callers pass the live preference so that changing it
 * re-measures every row at once — and shrinks total height below the current
 * scroll offset, which is what INV-6's clamp exists for.
 */
export function rowHeight(previewLines: number): number {
  const lines = Math.min(
    MAX_PREVIEW_LINES,
    Math.max(MIN_PREVIEW_LINES, Math.round(previewLines)),
  );
  return SINGLE_LINE_FLOOR + (lines - 1) * TITLE_LINE_PX;
}

/** Manifest §5.2: 240px of overscan above and below the viewport. TanStack
 *  counts rows rather than pixels, so convert at the call site. */
export const OVERSCAN_PX = 240;
export function overscanRows(previewLines: number): number {
  return Math.ceil(OVERSCAN_PX / rowHeight(previewLines));
}

/* ------------------------------------------------------------- constants --- */

/** §5.2. The daemon clamps with its own maximum. */
export const PAGE_SIZE = 200;
/** §5.2: fire load-more when the scroller is within 300px of the bottom. */
export const LOAD_MORE_THRESHOLD_PX = 300;
/** §5.3. */
export const SEARCH_LIMIT = 500;
export const SEARCH_DEBOUNCE_MS = 250;
export const UNDO_WINDOW_MS = 5000;
export const COPY_FLASH_MS = 700;
export const REVEAL_TIMEOUT_MS = 10_000;

/** §5.1. 3000ms while healthy; 5000ms while offline or erroring — never
 *  hammer a dead or initializing service. */
export const POLL_ACTIVE_MS = 3000;
export const POLL_BACKOFF_MS = 5000;
/** §5.1: status polls faster so "offline" surfaces within one cycle. */
export const STATUS_POLL_MS = 2000;
/** §5.1: `list_peers` at 10s — an online dot refresh without user
 *  interaction, decoupled from the status poll so it is ≤6 calls/min. */
export const PEERS_POLL_MS = 10_000;
