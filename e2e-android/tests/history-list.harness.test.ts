import { describe, expect, test } from "vitest";

import {
  itemRows,
  maxScrollTop,
  renderedRowsCoverViewport,
  reservesConservativeTextList,
  type ListSnapshot,
} from "../src/harness/list.js";

const SHRUNK_ARTIFACT: ListSnapshot = {
  frame: 3828,
  scrollTop: 4723,
  scrollHeight: 5130,
  clientHeight: 407,
  totalSize: 5130,
  rows: Array.from({ length: 9 }, (_, index) => ({
    id: `item-${113 - index}`,
    start: 4437 + index * 77,
    height: 77,
    active: false,
    text: `anchor item ${113 - index}`,
  })),
};

const PADDED_SHRINK_ARTIFACT: ListSnapshot = {
  frame: 252,
  scrollTop: 4849,
  scrollHeight: 5256,
  clientHeight: 407,
  totalSize: 5172,
  rows: Array.from({ length: 8 }, (_, index) => ({
    id: `item-${112 - index}`,
    start: 4556 + index * 77,
    height: 77,
    active: false,
    text: `anchor item ${112 - index}`,
  })),
};

describe("Android History artifact geometry", () => {
  test("recognises the actual clamp instead of a stale estimated total", () => {
    expect(maxScrollTop(SHRUNK_ARTIFACT)).toBe(4723);
    expect(SHRUNK_ARTIFACT.scrollTop).toBe(maxScrollTop(SHRUNK_ARTIFACT));
    expect(SHRUNK_ARTIFACT.totalSize).toBeGreaterThan(2799);
    expect(renderedRowsCoverViewport(SHRUNK_ARTIFACT)).toBe(true);
  });

  test("covers virtual content while excluding measured trailing padding", () => {
    expect(PADDED_SHRINK_ARTIFACT.scrollTop).toBe(
      maxScrollTop(PADDED_SHRINK_ARTIFACT),
    );
    expect(
      PADDED_SHRINK_ARTIFACT.scrollTop + PADDED_SHRINK_ARTIFACT.clientHeight,
    ).toBe(PADDED_SHRINK_ARTIFACT.scrollHeight);
    expect(PADDED_SHRINK_ARTIFACT.scrollHeight).toBeGreaterThan(
      PADDED_SHRINK_ARTIFACT.totalSize,
    );
    expect(renderedRowsCoverViewport(PADDED_SHRINK_ARTIFACT)).toBe(true);
  });

  test("rejects an unexplained gap between rendered rows", () => {
    const rows = PADDED_SHRINK_ARTIFACT.rows.map((row) => ({ ...row }));
    rows[5] = { ...rows[5]!, start: rows[5]!.start + 1 };
    expect(
      renderedRowsCoverViewport({ ...PADDED_SHRINK_ARTIFACT, rows }),
    ).toBe(false);
  });

  test("excludes the Today heading from intrinsic item geometry", () => {
    const rows = itemRows([
      { id: "", start: 0, height: 34, active: false, text: "Today" },
      {
        id: "short",
        start: 34,
        height: 77,
        active: false,
        text: "short",
      },
      {
        id: "long",
        start: 111,
        height: 119,
        active: false,
        text: "long long",
      },
    ]);

    expect(rows.map((row) => row.id)).toEqual(["short", "long"]);
    expect(rows.map((row) => row.height)).toEqual([77, 119]);
  });

  test("accepts measured rows inside the conservative 60-item reservation", () => {
    const snapshot: ListSnapshot = {
      frame: 1806,
      scrollTop: 0,
      scrollHeight: 6838,
      clientHeight: 407,
      totalSize: 6838,
      rows: [],
    };

    expect(
      reservesConservativeTextList(snapshot, 60, {
        group: 34,
        short: 77,
        long: 119,
      }),
    ).toBe(true);
  });
});
