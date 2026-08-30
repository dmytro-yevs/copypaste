import { describe, expect, test } from "vitest";

import {
  normalizeTouchCapabilityDiagnostic,
  readTouchCapabilityDiagnostic,
  touchCapabilityPageProbe,
  touchCapabilityDiagnosticJson,
} from "../src/harness/touch-capabilities.js";

const savedDocument = (globalThis as { document?: unknown }).document;
const savedWindow = (globalThis as { window?: unknown }).window;
const savedGetComputedStyle = globalThis.getComputedStyle;
const savedNavigator = Object.getOwnPropertyDescriptor(globalThis, "navigator");

function restoreGlobals(): void {
  if (savedDocument === undefined)
    delete (globalThis as { document?: unknown }).document;
  else (globalThis as { document?: unknown }).document = savedDocument;
  if (savedWindow === undefined)
    delete (globalThis as { window?: unknown }).window;
  else (globalThis as { window?: unknown }).window = savedWindow;
  globalThis.getComputedStyle = savedGetComputedStyle;
  if (savedNavigator === undefined) delete (globalThis as { navigator?: unknown }).navigator;
  else Object.defineProperty(globalThis, "navigator", savedNavigator);
}

describe("touch capability diagnostics", () => {
  test("preserves safe media facts and bounded geometry", () => {
    const parsed = JSON.parse(
      touchCapabilityDiagnosticJson({
        media: {
          pointerCoarse: true,
          pointerFine: false,
          pointerNone: false,
          anyPointerCoarse: true,
          anyPointerFine: false,
          hover: false,
          anyHover: false,
        },
        viewport: { width: 412, height: 915 },
        dataPointer: "coarse",
        maxTouchPoints: 5,
        tapMin: "44px",
        controls: [
          {
            present: true,
            tag: "button",
            classPresent: true,
            computedMinBlockSize: 44,
            actualRect: { width: 120, height: 44 },
          },
        ],
      }),
    );

    expect(parsed.media).toEqual({
      pointerCoarse: true,
      pointerFine: false,
      pointerNone: false,
      anyPointerCoarse: true,
      anyPointerFine: false,
      hover: false,
      anyHover: false,
    });
    expect(parsed.viewport).toEqual({ width: 412, height: 915 });
    expect(parsed.dataPointer).toBe("coarse");
    expect(parsed.maxTouchPoints).toBe(5);
    expect(parsed.tapMin).toEqual({ value: 44, unit: "px" });
    expect(parsed.controls[0]).toEqual({
      index: 0,
      present: true,
      tag: "button",
      classPresent: true,
      computedMinBlockSize: 44,
      actualRect: { width: 120, height: 44 },
    });
  });

  test("rejects invalid numbers, enums and arbitrary token strings", () => {
    const input = {
      media: { pointerCoarse: "yes", hover: "maybe" },
      dataPointer: "private-pointer",
      viewport: { width: Number.POSITIVE_INFINITY, height: -4 },
      tapMin: "var(--private-token-that-must-not-escape)",
      controls: [
        {
          present: "yes",
          tag: "script",
          classPresent: "yes",
          computedMinBlockSize: Number.NaN,
          actualRect: { width: Number.POSITIVE_INFINITY, height: -9 },
          label: "private control label",
        },
      ],
    };
    const normalized = normalizeTouchCapabilityDiagnostic(input);
    const json = JSON.stringify(normalized);

    expect(normalized.media.pointerCoarse).toBe(null);
    expect(normalized.media.hover).toBe(null);
    expect(normalized.viewport).toEqual({ width: null, height: 0 });
    expect(normalized.tapMin).toBe(null);
    expect(normalized.dataPointer).toBe(null);
    expect(normalized.maxTouchPoints).toBe(null);
    expect(normalized.controls[0]).toEqual({
      index: 0,
      present: false,
      tag: "other",
      classPresent: false,
      computedMinBlockSize: null,
      actualRect: null,
    });
    expect(json).not.toContain("private");
    expect(json).not.toContain("label");
  });

  test("runs the page callback and selects the reachable duplicate", () => {
    const hidden = {
      tagName: "BUTTON",
      classList: { length: 1 },
      contains: () => false,
      getBoundingClientRect: () => ({ left: 0, top: 0, width: 0, height: 0 }),
    } as unknown as Element;
    const visible = {
      tagName: "BUTTON",
      classList: { length: 0 },
      contains: (node: unknown) => node === visible,
      getBoundingClientRect: () => ({
        left: 4,
        top: 8,
        width: 120,
        height: 44,
      }),
    } as unknown as Element;
    const root = {
      clientWidth: 412,
      clientHeight: 915,
      dataset: { pointer: "coarse" },
    } as unknown as HTMLElement;
    (globalThis as { document?: unknown }).document = {
      documentElement: root,
      querySelectorAll: () => [hidden, visible],
      elementFromPoint: () => visible,
    };
    (globalThis as { window?: unknown }).window = {
      matchMedia: (query: string) => ({
        matches: query === "(pointer: coarse)" || query === "(any-pointer: coarse)",
      }),
    };
    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: { maxTouchPoints: 5 },
    });
    globalThis.getComputedStyle = (() => ({
      getPropertyValue: (name: string) =>
        name === "--tap-min" || name === "min-block-size" ? "44px" : "",
      minBlockSize: "44px",
    })) as unknown as typeof getComputedStyle;

    try {
      const snapshot = touchCapabilityPageProbe(["[aria-label=private]"]);
      expect(snapshot.media.pointerCoarse).toBe(true);
      expect(snapshot.media.pointerFine).toBe(false);
      expect(snapshot.media.hover).toBe(false);
      expect(snapshot.dataPointer).toBe("coarse");
      expect(snapshot.maxTouchPoints).toBe(5);
      expect(snapshot.controls).toEqual([
        {
          index: 0,
          present: true,
          tag: "button",
          classPresent: false,
          computedMinBlockSize: 44,
          actualRect: { width: 120, height: 44 },
        },
      ]);
    } finally {
      restoreGlobals();
    }
  });

  test("returns an unavailable snapshot when page capture fails", async () => {
    const app = {
      withPage: async () => {
        throw new Error("private page path");
      },
    } as never;
    const snapshot = await readTouchCapabilityDiagnostic(app, ["button"]);
    expect(snapshot.media.pointerCoarse).toBe(null);
    expect(snapshot.media.pointerFine).toBe(null);
    expect(snapshot.viewport).toEqual({ width: null, height: null });
    expect(snapshot.maxTouchPoints).toBe(null);
    expect(JSON.stringify(snapshot)).not.toContain("private");
  });
});
