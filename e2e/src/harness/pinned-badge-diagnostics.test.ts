import { JSDOM } from "jsdom";
import { afterEach, describe, expect, test, vi } from "vitest";

import type { Browser } from "./webview-guard.js";
import {
  pinnedBadgeSnapshot,
  readPagePinnedBadgeSnapshot,
  type PinnedBadgeSnapshot,
} from "./pinned-badge-diagnostics.js";
import { ROW } from "./ui.js";

let dom: JSDOM | undefined;

function mount(markup: string): void {
  dom = new JSDOM(`<main>${markup}</main>`);
  Object.defineProperty(globalThis, "document", {
    configurable: true,
    value: dom.window.document,
  });
  Object.defineProperty(globalThis, "getComputedStyle", {
    configurable: true,
    value: dom.window.getComputedStyle.bind(dom.window),
  });
}

function setBox(selector: string, width = 100, height = 24): void {
  const element = dom!.window.document.querySelector<HTMLElement>(selector)!;
  Object.defineProperty(element, "getBoundingClientRect", {
    configurable: true,
    value: () => ({
      left: 10,
      top: 20,
      width,
      height,
      right: 10 + width,
      bottom: 20 + height,
      x: 10,
      y: 20,
      toJSON: () => ({}),
    }),
  });
}

function rowMarkup(id: string, badge = '<span title="Pinned">Pinned</span>') {
  return `<div role="listitem" id="history-row-${id}">${badge}</div>`;
}

afterEach(() => {
  dom?.window.close();
  dom = undefined;
  Reflect.deleteProperty(globalThis, "document");
  Reflect.deleteProperty(globalThis, "getComputedStyle");
});

describe("pinned badge DOM observation", () => {
  test.each([
    ["missing row", rowMarkup("present"), "missing", 0, 0],
    ["missing badge", rowMarkup("missing", ""), "missing", 1, 0],
    [
      "duplicate badge",
      rowMarkup(
        "duplicate",
        '<span title="Pinned">one</span><span title="Pinned">two</span>',
      ),
      "duplicate",
      1,
      2,
    ],
  ])(
    "reports %s without a second DOM read",
    (_, markup, id, rowCount, badgeCount) => {
      mount(markup);
      const snapshot = readPagePinnedBadgeSnapshot([id], ROW);
      expect(snapshot.rows[0]).toMatchObject({ id, rowCount, badgeCount });
    },
  );

  test.each([
    ["zero area", "", 0, 24],
    ["display none", "display: none", 100, 24],
    ["visibility hidden", "visibility: hidden", 100, 24],
    ["visibility collapse", "visibility: collapse", 100, 24],
  ])(
    "rejects %s badge geometry or visibility",
    (_, style, width, height) => {
      mount(
        rowMarkup(
          "item",
          `<span title="Pinned" style="${style}">Pinned</span>`,
        ),
      );
      setBox("#history-row-item", 100, 40);
      setBox('[title="Pinned"]', width, height);
      const badge = readPagePinnedBadgeSnapshot(["item"], ROW).rows[0]!.badge!;
      expect(badge.displayed).toBe(false);
      expect(badge.rect).toMatchObject({ width, height });
    },
  );

  test("uses one browser execution for the atomic DOM callback", async () => {
    mount(rowMarkup("item"));
    setBox("#history-row-item", 100, 40);
    setBox('[title="Pinned"]');
    const execute = vi.fn(
      async (
        callback: (
          ids: readonly string[],
          rowSelector: string,
        ) => PinnedBadgeSnapshot,
        ids: readonly string[],
        rowSelector: string,
      ) => callback(ids, rowSelector),
    );

    const snapshot = await pinnedBadgeSnapshot(
      { execute } as unknown as Browser,
      ["item"],
    );

    expect(execute).toHaveBeenCalledTimes(1);
    expect(snapshot.rows[0]?.badge?.displayed).toBe(true);
  });

  test("rejects an ancestor with zero opacity", () => {
    mount('<div style="opacity: 0">' + rowMarkup("item") + "</div>");
    setBox("#history-row-item", 100, 40);
    setBox('[title="Pinned"]');
    const row = readPagePinnedBadgeSnapshot(["item"], ROW).rows[0]!;
    expect(row.row?.displayed).toBe(false);
    expect(row.badge?.displayed).toBe(false);
  });

  test("reports detached rows and follows stable ids after reorder", () => {
    mount(rowMarkup("first") + rowMarkup("second"));
    const first = dom!.window.document.getElementById("history-row-first")!;
    first.remove();
    expect(readPagePinnedBadgeSnapshot(["first"], ROW).rows[0]).toMatchObject({
      id: "first",
      rowCount: 0,
      badgeCount: 0,
    });
    dom!.window.document.querySelector("main")!.prepend(first);
    const second = dom!.window.document.getElementById("history-row-second")!;
    second.remove();
    dom!.window.document.querySelector("main")!.prepend(second);
    setBox("#history-row-first", 100, 40);
    setBox("#history-row-second", 100, 40);
    setBox("#history-row-first [title=Pinned]");
    setBox("#history-row-second [title=Pinned]");

    const snapshot = readPagePinnedBadgeSnapshot(["first", "second"], ROW);
    expect(snapshot.renderedRowIds).toEqual(["second", "first"]);
    expect(snapshot.rows).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: "first", rowCount: 1, badgeCount: 1 }),
        expect.objectContaining({ id: "second", rowCount: 1, badgeCount: 1 }),
      ]),
    );
  });
});
