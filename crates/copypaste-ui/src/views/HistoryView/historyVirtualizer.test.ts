/**
 * historyVirtualizer.test.ts — rowHeightFor box-model reconciliation
 * (CopyPaste-g27b.30, verified alongside CopyPaste-g27b.25).
 *
 * The headless geometry audit found every single-line row's `.row__meta`
 * bottom edge sitting 2-3px BELOW the next row's top (adjacent [role=option]
 * rects overlapping site-wide) because rowHeightFor's allocated height was
 * smaller than the real rendered box: .row__title line (21px) + .row__meta
 * margin-top (2px) + .row__meta line (~18px) + .row's own vertical padding
 * (18px, tokens.css --pad-row: 9px 10px) ≈ 59px, versus the old 34px
 * "comfortable" base. These tests assert the reconciled floor directly
 * against that real box model so a future edit can't silently regress it
 * below the point where rows visually overlap again.
 */
import { describe, expect, it } from "vitest";
import type { HistoryEntry } from "../../lib/ipc";
import { rowHeightFor } from "./historyVirtualizer";

// Local fixture (production code must not import src/lib/fixtures/** outside
// mockIpc.ts / GalleryView — see fixtures/index.ts's import-boundary rule and
// its importBoundary.test.ts grep gate). Mirrors HistoryRow.test.tsx's entry().
const entry = (over: Partial<HistoryEntry> = {}): HistoryEntry => ({
  id: "1",
  content_type: "text/plain",
  preview: "hello world",
  is_sensitive: false,
  wall_time: 1_700_000_000_000,
  pinned: false,
  ...over,
});

// The real box every single-line text row renders, regardless of density
// (patterns.css's `.row` rule does not vary by density — the axis is frozen
// to "comfortable" in HistoryView.tsx): title line (21px) + meta margin-top
// (2px) + meta line (rounded up to 18px) + row padding (18px) = 59px.
const REAL_SINGLE_LINE_BOX = 59;

describe("rowHeightFor — text rows (g27b.30 overlap fix)", () => {
  it("comfortable (default) density allocates at least the real rendered box", () => {
    const h = rowHeightFor(entry(), /* previewSize */ 0, /* imageMaxHeight */ 40);
    expect(h).toBeGreaterThanOrEqual(REAL_SINGLE_LINE_BOX);
  });

  it("spacious and compact never allocate less than the real rendered box either — the CSS box is IDENTICAL across density values (the axis is frozen), so under-allocating any of the three reproduces the site-wide overlap", () => {
    const spacious = rowHeightFor(entry(), 0, 40, "spacious");
    const compact = rowHeightFor(entry(), 0, 40, "compact");
    expect(spacious).toBeGreaterThanOrEqual(REAL_SINGLE_LINE_BOX);
    expect(compact).toBeGreaterThanOrEqual(REAL_SINGLE_LINE_BOX);
  });

  it("previous under-allocating bases (28/34/42) are gone: a set of consecutive default rows never overlaps — stacking N rows of this height must leave a non-negative gap versus the real content height", () => {
    // Regression guard for the exact audit symptom: metaToNextTitleGap was
    // -3px because allocated height (34) < real content (59.25ish). Assert
    // the margin directly instead of re-deriving it, so the test fails loudly
    // if the constant is ever dropped back below the real box.
    const allocated = rowHeightFor(entry({ preview: "short" }), 0, 40);
    const gap = allocated - REAL_SINGLE_LINE_BOX;
    expect(gap).toBeGreaterThanOrEqual(0);
  });

  it("previewSize can still grow the row taller than the floor (explicit user override), never shrink it below the floor", () => {
    const tall = rowHeightFor(entry(), /* previewSize */ 200, 40);
    expect(tall).toBe(200);
    const short = rowHeightFor(entry(), /* previewSize */ 10, 40);
    expect(short).toBeGreaterThanOrEqual(REAL_SINGLE_LINE_BOX);
  });

  it("previewLines=1 (default) returns exactly the single-line height — no multi-line allocation leaks in", () => {
    const oneLine = rowHeightFor(entry({ preview: "x".repeat(500) }), 0, 40, "comfortable", 1);
    expect(oneLine).toBe(REAL_SINGLE_LINE_BOX);
  });

  it("previewLines>1 always reserves the FULL configured line cap (lineCount*lineHeight), never a width-guessed under-estimate", () => {
    // A short clip that a width-agnostic char-count heuristic would estimate
    // as "1 line" must still get the full previewLines=4 allocation, because
    // at a narrow container width it can legitimately wrap to more lines than
    // a wide-desktop char-count guess predicts.
    const short = rowHeightFor(entry({ preview: "short clip" }), 0, 40, "comfortable", 4);
    const oneLine = rowHeightFor(entry({ preview: "short clip" }), 0, 40, "comfortable", 1);
    expect(short).toBe(oneLine + 3 * 21); // TITLE_LINE_PX = 21
  });
});

describe("rowHeightFor — image rows (g27b.25 verify + consistency)", () => {
  it("comfortable density reserves at least imageMaxHeight + the row's real vertical padding (18px, --pad-row 9px 10px)", () => {
    const h = rowHeightFor(entry({ content_type: "image/png" }), 0, 100, "comfortable");
    expect(h).toBeGreaterThanOrEqual(100 + 18);
  });

  it("compact density (previously under-allocating by 10px) also reserves at least imageMaxHeight + 18px", () => {
    const h = rowHeightFor(entry({ content_type: "image/png" }), 0, 100, "compact");
    expect(h).toBeGreaterThanOrEqual(100 + 18);
  });

  it("small imageMaxHeight values still respect the 34px floor", () => {
    const h = rowHeightFor(entry({ content_type: "image/png" }), 0, 4, "comfortable");
    expect(h).toBeGreaterThanOrEqual(34);
  });
});

describe("rowHeightFor — file rows (unaffected by the g27b.30 change)", () => {
  it("stays fixed at 44px regardless of density/previewLines", () => {
    expect(rowHeightFor(entry({ content_type: "file" }), 0, 40, "compact", 3)).toBe(44);
  });
});
