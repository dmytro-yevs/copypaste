/**
 * INV-5 / AT-8 — row heights over-reserve, and are never a function of content.
 *
 * The "smarter" character-count estimate was itself the bug
 * (CopyPaste-g27b.30): it under-reserved at narrow widths and rows overlapped
 * site-wide. These tests pin the two properties that make under-reservation
 * impossible — the height depends only on the setting, and it is at least the
 * full cap.
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
  overscanRows,
  rowHeight,
} from "./layout";

describe("row height reserves the full preview cap", () => {
  it("is the floor plus one title line per extra preview line", () => {
    for (let lines = MIN_PREVIEW_LINES; lines <= MAX_PREVIEW_LINES; lines++) {
      expect(rowHeight(lines)).toBe(
        SINGLE_LINE_FLOOR + (lines - 1) * TITLE_LINE_PX,
      );
    }
  });

  it("leaves room for the title, the meta line and both paddings", () => {
    // A row that reserves less than its own parts overlaps its neighbour.
    expect(SINGLE_LINE_FLOOR).toBe(
      TITLE_LINE_PX + META_MARGIN_PX + META_LINE_PX + ROW_PAD_V,
    );
    expect(rowHeight(1)).toBeGreaterThan(TITLE_LINE_PX + META_LINE_PX);
  });

  it("is strictly increasing in the preview-line setting", () => {
    for (let lines = MIN_PREVIEW_LINES; lines < MAX_PREVIEW_LINES; lines++) {
      expect(rowHeight(lines + 1)).toBeGreaterThan(rowHeight(lines));
    }
  });

  it("clamps a setting outside the slider's range instead of shrinking", () => {
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

describe("overscan is a pixel budget converted to rows", () => {
  it("covers at least 240px above and below at every setting", () => {
    for (let lines = MIN_PREVIEW_LINES; lines <= MAX_PREVIEW_LINES; lines++) {
      expect(overscanRows(lines) * rowHeight(lines)).toBeGreaterThanOrEqual(
        OVERSCAN_PX,
      );
    }
  });

  it("renders fewer rows as rows get taller", () => {
    expect(overscanRows(MAX_PREVIEW_LINES)).toBeLessThan(
      overscanRows(MIN_PREVIEW_LINES),
    );
  });
});
