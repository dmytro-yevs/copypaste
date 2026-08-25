/**
 * INV-5 / AT-8 — row heights over-reserve, and are never a function of content.
 *
 * The "smarter" character-count estimate was itself the bug
 * (CopyPaste-g27b.30): it under-reserved at narrow widths and rows overlapped
 * site-wide. These tests pin the two properties that make under-reservation
 * impossible — every row reserves the fixed three-line cap.
 */
import { describe, expect, it } from "vitest";

import {
  MAX_PREVIEW_LINES,
  META_LINE_PX,
  META_MARGIN_PX,
  MIN_PREVIEW_LINES,
  OVERSCAN_PX,
  ROW_PAD_V,
  SINGLE_LINE_FLOOR,
  TITLE_LINE_PX,
  IMAGE_PREVIEW_HEIGHT_MULTIPLIER,
  imagePreviewHeight,
  imageRowHeight,
  overscanRows,
  rowPreviewHeight,
  rowHeight,
} from "@/features/history/model/virtualizationMetrics";

describe("row height reserves the full preview cap", () => {
  it("reserves the fixed three-line preview", () => {
    expect(MIN_PREVIEW_LINES).toBe(3);
    expect(MAX_PREVIEW_LINES).toBe(3);
    expect(rowHeight(3)).toBe(SINGLE_LINE_FLOOR + 2 * TITLE_LINE_PX);
  });

  it("leaves room for the title, the meta line and both paddings", () => {
    // A row that reserves less than its own parts overlaps its neighbour.
    expect(SINGLE_LINE_FLOOR).toBe(
      TITLE_LINE_PX + META_MARGIN_PX + META_LINE_PX + ROW_PAD_V,
    );
    expect(rowHeight(1)).toBeGreaterThan(TITLE_LINE_PX + META_LINE_PX);
  });

  it("clamps legacy and corrupt settings to three lines", () => {
    // A corrupt pref must not be able to produce a row shorter than one line.
    expect(rowHeight(0)).toBe(rowHeight(MIN_PREVIEW_LINES));
    expect(rowHeight(-4)).toBe(rowHeight(MIN_PREVIEW_LINES));
    expect(rowHeight(99)).toBe(rowHeight(MAX_PREVIEW_LINES));
  });

  it("takes no content argument at all", () => {
    // The signature is the guarantee: there is no item in scope to measure, so
    // no future edit can make the reservation content-dependent without
    // changing this call site.
    expect(rowHeight.length).toBe(1);
  });
});

describe("image preview height", () => {
  it("uses a 3.75× cap while reserving a matching virtual row height", () => {
    expect(imagePreviewHeight(3)).toBe(
      rowPreviewHeight(3) * IMAGE_PREVIEW_HEIGHT_MULTIPLIER,
    );
    expect(imageRowHeight(3)).toBe(
      imagePreviewHeight(3) + META_MARGIN_PX + META_LINE_PX + ROW_PAD_V,
    );
  });
});

describe("overscan is a pixel budget converted to rows", () => {
  it("covers at least 240px above and below", () => {
    expect(overscanRows(3) * rowHeight(3)).toBeGreaterThanOrEqual(
      OVERSCAN_PX,
    );
  });
});
