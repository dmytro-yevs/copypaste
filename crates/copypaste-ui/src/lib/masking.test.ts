import { describe, expect, it } from "vitest";
import { applySpanMasking, shouldMask } from "./masking";
import { buildOffsets, computeVisibleWindow } from "../views/HistoryView";

// ---------------------------------------------------------------------------
// applySpanMasking — code-point-correct sensitive-span redaction
// ---------------------------------------------------------------------------

describe("applySpanMasking", () => {
  it("returns the text unchanged when there are no spans", () => {
    expect(applySpanMasking("hello world", [])).toBe("hello world");
  });

  it("masks a single span with bullets of equal code-point length", () => {
    // mask "world" (indices 6..11)
    expect(applySpanMasking("hello world", [[6, 11]])).toBe("hello •••••");
  });

  it("masks multiple, out-of-order spans left-to-right", () => {
    // token "abc-XYZ": mask [0,3] and [4,7]
    expect(applySpanMasking("abc-XYZ", [[4, 7], [0, 3]])).toBe("•••-•••");
  });

  it("clamps spans that run past the end of the text", () => {
    expect(applySpanMasking("secret", [[0, 999]])).toBe("••••••");
  });

  it("counts astral (emoji) characters as single code points", () => {
    // "a😀b": code points are [a, 😀, b]. Masking [1,2] hides only the emoji.
    const masked = applySpanMasking("a😀b", [[1, 2]]);
    expect(masked).toBe("a•b");
  });

  it("does not reveal secret material when overlapping spans are given", () => {
    // overlapping [0,4] and [2,6] over "password" → first 6 chars masked
    expect(applySpanMasking("password", [[0, 4], [2, 6]])).toBe("••••••rd");
  });
});

// ---------------------------------------------------------------------------
// shouldMask — blur/redaction gate
// ---------------------------------------------------------------------------

describe("shouldMask", () => {
  it("returns true when maskSensitive is on and entry is sensitive", () => {
    expect(shouldMask({ is_sensitive: true }, true)).toBe(true);
  });

  it("returns false when maskSensitive is off, even if entry is sensitive", () => {
    // User disabled masking → show content unblurred.
    expect(shouldMask({ is_sensitive: true }, false)).toBe(false);
  });

  it("returns false when entry is not sensitive, even if maskSensitive is on", () => {
    // Non-sensitive entries are never blurred.
    expect(shouldMask({ is_sensitive: false }, true)).toBe(false);
  });

  it("returns false when both maskSensitive is off and entry is not sensitive", () => {
    expect(shouldMask({ is_sensitive: false }, false)).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Virtualization math (Fix #1)
// ---------------------------------------------------------------------------

describe("buildOffsets", () => {
  it("produces a prefix-sum table with a leading zero and trailing total", () => {
    expect(buildOffsets([10, 20, 30])).toEqual([0, 10, 30, 60]);
  });

  it("handles an empty list", () => {
    expect(buildOffsets([])).toEqual([0]);
  });
});

describe("computeVisibleWindow", () => {
  // 100 rows of 28px each → offsets length 101, total height 2800.
  const heights = new Array(100).fill(28);
  const offsets = buildOffsets(heights);

  it("returns an empty range for an empty list", () => {
    expect(computeVisibleWindow([0], 0, 500)).toEqual({ start: 0, end: 0 });
  });

  it("renders only the top rows (plus overscan) when scrolled to the top", () => {
    const { start, end } = computeVisibleWindow(offsets, 0, 280, 0);
    expect(start).toBe(0);
    // viewport 280px / 28px = 10 rows visible, no overscan
    expect(end).toBe(10);
    // crucially, far fewer than all 100 rows are rendered
    expect(end - start).toBeLessThan(heights.length);
  });

  it("windows to the middle of the list when scrolled down", () => {
    // scroll to row 50 (offset 1400), viewport 280px, no overscan
    const { start, end } = computeVisibleWindow(offsets, 1400, 280, 0);
    expect(start).toBe(50);
    expect(end).toBe(60);
  });

  it("includes an overscan buffer above and below the viewport", () => {
    const noBuffer = computeVisibleWindow(offsets, 1400, 280, 0);
    const buffered = computeVisibleWindow(offsets, 1400, 280, 56); // 2 rows
    expect(buffered.start).toBeLessThan(noBuffer.start);
    expect(buffered.end).toBeGreaterThan(noBuffer.end);
  });

  it("supports mixed row heights (image rows taller than text rows)", () => {
    // rows: text(28), image(40), text(28), image(40) → offsets [0,28,68,96,136]
    const mixed = buildOffsets([28, 40, 28, 40]);
    // viewport showing only the first two rows
    const { start, end } = computeVisibleWindow(mixed, 0, 60, 0);
    expect(start).toBe(0);
    expect(end).toBe(2);
  });
});
