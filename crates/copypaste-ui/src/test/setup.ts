/**
 * jsdom has no layout engine: every element reports a zero-sized box and there
 * is no `ResizeObserver`. A virtualised list asks how tall its viewport is and
 * renders nothing when the answer is zero, so without the fakes below every
 * list test would pass vacuously — which is worse than failing.
 *
 * The smallest fake that makes the geometry real: a fixed 800px viewport, a
 * `ResizeObserver` that never fires, and a matching `getBoundingClientRect`.
 * Row *heights* are not faked — they come from `rowHeight()`, the number under
 * test (INV-5).
 */
import { afterEach, vi } from "vitest";
import { cleanup } from "@testing-library/react";

// `globals: false` means React Testing Library does not register its own
// cleanup, so a second render in the same file would stack on the first.
afterEach(cleanup);

const VIEWPORT_PX = 800;

class NoopResizeObserver implements ResizeObserver {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
}

vi.stubGlobal("ResizeObserver", NoopResizeObserver);

const storage = new Map<string, string>();
const localStorageStub: Storage = {
  get length() {
    return storage.size;
  },
  clear: () => storage.clear(),
  getItem: (key) => storage.get(key) ?? null,
  key: (index) => [...storage.keys()][index] ?? null,
  removeItem: (key) => storage.delete(key),
  setItem: (key, value) => storage.set(key, String(value)),
};

Object.defineProperty(window, "localStorage", {
  configurable: true,
  value: localStorageStub,
});
vi.stubGlobal("localStorage", localStorageStub);

for (const [property, value] of [
  ["clientHeight", VIEWPORT_PX],
  ["clientWidth", VIEWPORT_PX],
  ["offsetHeight", VIEWPORT_PX],
  ["offsetWidth", VIEWPORT_PX],
] as const) {
  Object.defineProperty(HTMLElement.prototype, property, {
    configurable: true,
    get(this: HTMLElement) {
      // A styled height wins, so a test that sets one is believed.
      const styled = Number.parseFloat(
        this.style[property.includes("Height") ? "height" : "width"],
      );
      return Number.isFinite(styled) ? styled : value;
    },
  });
}

Object.defineProperty(HTMLElement.prototype, "scrollHeight", {
  configurable: true,
  writable: true,
  value: VIEWPORT_PX * 10,
});

HTMLElement.prototype.getBoundingClientRect = function getBoundingClientRect() {
  return {
    x: 0,
    y: 0,
    top: 0,
    left: 0,
    right: VIEWPORT_PX,
    bottom: VIEWPORT_PX,
    width: VIEWPORT_PX,
    height: VIEWPORT_PX,
    toJSON: () => ({}),
  } as DOMRect;
};

// jsdom implements neither, and Radix uses both for pointer-driven dismissal.
if (!Element.prototype.hasPointerCapture) {
  Element.prototype.hasPointerCapture = () => false;
  Element.prototype.setPointerCapture = () => {};
  Element.prototype.releasePointerCapture = () => {};
}
if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = () => {};
}
