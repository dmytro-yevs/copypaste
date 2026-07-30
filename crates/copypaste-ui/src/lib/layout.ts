/**
 * Row geometry for the virtualiser — manifest 06 §3.1.3 and §5.2.
 *
 * INV-5: **row heights are over-reserved, never estimated from the content.**
 * The rendered height of a preview depends on the pane width, which a pure
 * height function cannot know, so every text row reserves the full
 * `PREVIEW_LINES` cap whether or not the clip fills it. The "smarter"
 * character-count estimate was itself the bug (CopyPaste-g27b.30): it
 * under-reserved at narrow widths and rows overlapped site-wide.
 *
 * The row renders inside a box of exactly this height with `overflow: hidden`,
 * so the reservation is an upper bound by construction, not by arithmetic.
 *
 * The density axis ("compact" / "comfortable" / "spacious") is not ported:
 * production froze it to "comfortable" and manifest §9.2 says collapse it.
 */

/** Row vertical padding, both halves. Derived from `--pad-row-y: 9px`. */
export const ROW_PAD_V = 18;

/** `--fs-base` 14px x `--lh-normal` 1.5. */
export const TITLE_LINE_PX = 21;

/** Meta line height, and its margin above the title. */
export const META_LINE_PX = 18;
export const META_MARGIN_PX = 2;

/** 21 + 2 + 18 + 18. Guarantees a strictly positive gap to the next row. */
export const SINGLE_LINE_FLOOR =
  TITLE_LINE_PX + META_MARGIN_PX + META_LINE_PX + ROW_PAD_V;

/** Preview lines a text row reserves. v1 exposed this as a 1-6 slider; v2 has
 *  no settings screen, so it is one constant that the height and the CSS
 *  line-clamp both read. */
export const PREVIEW_LINES = 2;

/**
 * The reserved height of a row. Constant by design: every text row gets the
 * full cap (INV-5). It is a function rather than a bare constant so that a
 * second content kind — an image or file row, which manifest §3.1.3 sizes
 * differently — changes this one place and nothing else.
 */
export function rowHeight(): number {
  return SINGLE_LINE_FLOOR + (PREVIEW_LINES - 1) * TITLE_LINE_PX;
}

export const ROW_HEIGHT = rowHeight();

/** Manifest §5.2: 240px of overscan above and below the viewport. TanStack
 *  counts rows rather than pixels, so convert once, here. */
export const OVERSCAN_PX = 240;
export const OVERSCAN_ROWS = Math.ceil(OVERSCAN_PX / ROW_HEIGHT);

/** Manifest §5.2 / §5.3. */
export const PAGE_SIZE = 200;
export const SEARCH_LIMIT = 500;
export const SEARCH_DEBOUNCE_MS = 250;
export const UNDO_WINDOW_MS = 5000;
export const COPY_FLASH_MS = 700;
export const REVEAL_TIMEOUT_MS = 10_000;
