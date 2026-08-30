import type { AndroidApp } from "./app.js";

const MAX_NUMBER = 10_000;
const MAX_CONTROLS = 32;
const MAX_CSS_LENGTH = 32;
const CSS_UNITS = ["px", "rem", "em", "vh", "vw", "%"] as const;
type CssUnit = (typeof CSS_UNITS)[number];

export interface TouchCapabilityDiagnostic {
  media: {
    pointerCoarse: boolean | null;
    pointerFine: boolean | null;
    pointerNone: boolean | null;
    anyPointerCoarse: boolean | null;
    anyPointerFine: boolean | null;
    hover: boolean | null;
    anyHover: boolean | null;
  };
  dataPointer: "coarse" | "fine" | null;
  maxTouchPoints: number | null;
  viewport: { width: number | null; height: number | null };
  tapMin: { value: number; unit: CssUnit } | null;
  controls: ReadonlyArray<{
    index: number;
    present: boolean;
    tag: "button" | "input" | "select" | "textarea" | "a" | "other" | null;
    classPresent: boolean;
    computedMinBlockSize: number | null;
    actualRect: { width: number; height: number } | null;
  }>;
}

function finiteNumber(value: unknown): number | null {
  if (typeof value !== "number" || !Number.isFinite(value)) return null;
  return Math.min(MAX_NUMBER, Math.max(0, value));
}

function nullableBoolean(value: unknown): boolean | null {
  return typeof value === "boolean" ? value : null;
}

function normalizeTag(
  value: unknown,
): TouchCapabilityDiagnostic["controls"][number]["tag"] {
  if (
    value === "button" ||
    value === "input" ||
    value === "select" ||
    value === "textarea" ||
    value === "a"
  ) {
    return value;
  }
  if (value === "other") return value;
  return value === null ? null : "other";
}

function normalizeCssLength(
  value: unknown,
): TouchCapabilityDiagnostic["tapMin"] {
  let number: unknown;
  let unit: unknown;
  if (typeof value === "string") {
    if (value.length > MAX_CSS_LENGTH) return null;
    const match = /^\s*(-?(?:\d+\.?\d*|\.\d+))\s*(px|rem|em|vh|vw|%)\s*$/u.exec(
      value,
    );
    number = match?.[1];
    unit = match?.[2];
  } else if (value !== null && typeof value === "object") {
    number = (value as { value?: unknown }).value;
    unit = (value as { unit?: unknown }).unit;
  }
  if (typeof number === "string" && number.length > 16) return null;
  const parsed = typeof number === "number" ? number : Number(number);
  if (
    !Number.isFinite(parsed) ||
    parsed < 0 ||
    parsed > MAX_NUMBER ||
    !CSS_UNITS.includes(unit as CssUnit)
  ) {
    return null;
  }
  return { value: parsed, unit: unit as CssUnit };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object";
}

export function normalizeTouchCapabilityDiagnostic(
  value: unknown,
): TouchCapabilityDiagnostic {
  const input = isRecord(value) ? value : {};
  const media = isRecord(input.media) ? input.media : {};
  const viewport = isRecord(input.viewport) ? input.viewport : {};
  const controls = Array.isArray(input.controls) ? input.controls : [];
  return {
    media: {
      pointerCoarse: nullableBoolean(media.pointerCoarse),
      pointerFine: nullableBoolean(media.pointerFine),
      pointerNone: nullableBoolean(media.pointerNone),
      anyPointerCoarse: nullableBoolean(media.anyPointerCoarse),
      anyPointerFine: nullableBoolean(media.anyPointerFine),
      hover: nullableBoolean(media.hover),
      anyHover: nullableBoolean(media.anyHover),
    },
    dataPointer:
      input.dataPointer === "coarse" || input.dataPointer === "fine"
        ? input.dataPointer
        : null,
    maxTouchPoints: finiteNumber(input.maxTouchPoints),
    viewport: {
      width: finiteNumber(viewport.width),
      height: finiteNumber(viewport.height),
    },
    tapMin: normalizeCssLength(input.tapMin),
    controls: controls.slice(0, MAX_CONTROLS).map((entry, index) => {
      const control = isRecord(entry) ? entry : {};
      const rect = isRecord(control.actualRect) ? control.actualRect : null;
      const width = rect ? finiteNumber(rect.width) : null;
      const height = rect ? finiteNumber(rect.height) : null;
      return {
        index,
        present: control.present === true,
        tag: normalizeTag(control.tag),
        classPresent: control.classPresent === true,
        computedMinBlockSize: finiteNumber(control.computedMinBlockSize),
        actualRect:
          width === null || height === null ? null : { width, height },
      };
    }),
  };
}

/**
 * This function is passed directly to Puppeteer. Keep every helper it needs
 * inside the callback so page evaluation never depends on a module closure.
 */
export function touchCapabilityPageProbe(knownSelectors: readonly string[]) {
  const max = 10_000;
  const css = (value: unknown): { value: number; unit: string } | null => {
    if (typeof value !== "string" || value.length > 32) return null;
    const match = /^\s*(\d+(?:\.\d+)?)\s*(px|rem|em|vh|vw|%)\s*$/u.exec(value);
    if (!match) return null;
    const number = Number(match[1]);
    return Number.isFinite(number) && number <= max
      ? { value: number, unit: match[2]! }
      : null;
  };
  const number = (value: unknown): number | null =>
    typeof value === "number" && Number.isFinite(value)
      ? Math.min(max, Math.max(0, value))
      : null;
  const media = (query: string): boolean | null => {
    try {
      return typeof window.matchMedia === "function"
        ? window.matchMedia(query).matches === true
        : null;
    } catch {
      return null;
    }
  };
  const tag = (element: Element): string => {
    const name = element.tagName.toLowerCase();
    return ["button", "input", "select", "textarea", "a"].includes(name)
      ? name
      : "other";
  };
  const root = document.documentElement;
  const rootStyle = root ? getComputedStyle(root) : null;
  const dataPointer = root?.dataset.pointer;
  const selectors = Array.isArray(knownSelectors)
    ? knownSelectors.slice(0, 32)
    : [];
  return {
    media: {
      pointerCoarse: media("(pointer: coarse)"),
      pointerFine: media("(pointer: fine)"),
      pointerNone: media("(pointer: none)"),
      anyPointerCoarse: media("(any-pointer: coarse)"),
      anyPointerFine: media("(any-pointer: fine)"),
      hover: media("(hover: hover)"),
      anyHover: media("(any-hover: hover)"),
    },
    dataPointer:
      dataPointer === "coarse" || dataPointer === "fine" ? dataPointer : null,
    maxTouchPoints: number(globalThis.navigator?.maxTouchPoints),
    viewport: {
      width: number(document.documentElement.clientWidth),
      height: number(document.documentElement.clientHeight),
    },
    tapMin: css(rootStyle?.getPropertyValue("--tap-min") ?? ""),
    controls: selectors.map((selector, index) => {
      let matches: Element[] = [];
      try {
        if (typeof selector === "string") {
          matches = Array.from(document.querySelectorAll(selector)).slice(0, 8);
        }
      } catch {
        matches = [];
      }
      const displayed = (element: Element): boolean => {
        const rect = element.getBoundingClientRect();
        if (rect.width <= 0 || rect.height <= 0) return false;
        const x = rect.left + rect.width / 2;
        const y = rect.top + rect.height / 2;
        return element.contains(document.elementFromPoint(x, y));
      };
      const element = matches.find(displayed) ?? null;
      if (!element) {
        return {
          index,
          present: false,
          tag: null,
          classPresent: false,
          computedMinBlockSize: null,
          actualRect: null,
        };
      }
      const style = getComputedStyle(element);
      const rect = element.getBoundingClientRect();
      const minBlockSize = css(
        style.getPropertyValue("min-block-size") || style.minBlockSize,
      );
      const width = number(rect.width);
      const height = number(rect.height);
      return {
        index,
        present: true,
        tag: tag(element),
        classPresent: element.classList.length > 0,
        computedMinBlockSize:
          minBlockSize?.unit === "px" ? minBlockSize.value : null,
        actualRect:
          width === null || height === null ? null : { width, height },
      };
    }),
  };
}

export async function readTouchCapabilityDiagnostic(
  app: AndroidApp,
  selectors: readonly string[],
): Promise<TouchCapabilityDiagnostic> {
  try {
    const snapshot = await app.withPage((page) =>
      page.evaluate(
        touchCapabilityPageProbe,
        [...selectors].slice(0, MAX_CONTROLS),
      ),
    );
    return normalizeTouchCapabilityDiagnostic(snapshot);
  } catch {
    return normalizeTouchCapabilityDiagnostic(null);
  }
}

export function touchCapabilityDiagnosticJson(value: unknown): string {
  return JSON.stringify(normalizeTouchCapabilityDiagnostic(value));
}
