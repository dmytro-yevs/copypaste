import { afterEach, describe, expect, it, vi } from "vitest";

import type { Appearance } from "@/lib/theme";
import { applyAppearance } from "@/lib/theme";
import { changeAppearanceFrom } from "@/lib/themeTransition";

const root = document.documentElement;
const originalMatchMedia = Object.getOwnPropertyDescriptor(window, "matchMedia");
const originalRequestAnimationFrame = Object.getOwnPropertyDescriptor(
  window,
  "requestAnimationFrame",
);
const originalStartViewTransition = Object.getOwnPropertyDescriptor(
  document,
  "startViewTransition",
);
const originalAnimate = Object.getOwnPropertyDescriptor(root, "animate");

const MIDNIGHT: Appearance = {
  theme: "dark",
  colorTheme: "midnight",
  translucency: false,
};

function restoreProperty(
  target: object,
  name: PropertyKey,
  descriptor: PropertyDescriptor | undefined,
): void {
  if (descriptor) Object.defineProperty(target, name, descriptor);
  else Reflect.deleteProperty(target, name);
}

function matchMedia(reduced: boolean): void {
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    value: vi.fn((query: string) => ({
      matches: query === "(prefers-reduced-motion: reduce)" && reduced,
    } as MediaQueryList)),
  });
}

function trigger(): HTMLElement {
  const button = document.createElement("button");
  vi.spyOn(button, "getBoundingClientRect").mockReturnValue({
    x: 10,
    y: 20,
    top: 20,
    left: 10,
    right: 50,
    bottom: 60,
    width: 40,
    height: 40,
    toJSON: () => ({}),
  });
  document.body.appendChild(button);
  return button;
}

function transitionEnd(element: Element, propertyName: string): void {
  const event = new Event("transitionend");
  Object.defineProperty(event, "propertyName", { value: propertyName });
  element.dispatchEvent(event);
}

afterEach(() => {
  vi.restoreAllMocks();
  restoreProperty(window, "matchMedia", originalMatchMedia);
  restoreProperty(window, "requestAnimationFrame", originalRequestAnimationFrame);
  restoreProperty(document, "startViewTransition", originalStartViewTransition);
  restoreProperty(root, "animate", originalAnimate);
  root.removeAttribute("style");
  delete root.dataset.themeTransition;
  document.body.replaceChildren();
});

describe("theme application transition", () => {
  it("does not animate an active theme and applies reduced-motion changes instantly", async () => {
    matchMedia(false);
    applyAppearance(MIDNIGHT);
    const start = vi.fn();
    Object.defineProperty(document, "startViewTransition", {
      configurable: true,
      value: start,
    });
    const repeatedUpdate = vi.fn();

    await changeAppearanceFrom(trigger(), () => MIDNIGHT, repeatedUpdate);

    expect(repeatedUpdate).not.toHaveBeenCalled();
    expect(start).not.toHaveBeenCalled();

    matchMedia(true);
    const update = vi.fn();
    await changeAppearanceFrom(
      trigger(),
      () => ({ ...MIDNIGHT, colorTheme: "aurora" }),
      update,
    );

    expect(update).toHaveBeenCalledOnce();
    expect(root.dataset.theme).toBe("aurora");
    expect(start).not.toHaveBeenCalled();
  });

  it("covers the old UI with a target-theme veil before committing the fallback", async () => {
    matchMedia(false);
    applyAppearance(MIDNIGHT);
    root.style.setProperty("--dur-theme", "300ms");
    root.style.setProperty("--dur-fast", "150ms");
    root.style.setProperty("--ease", "ease-out");
    root.style.setProperty("--ease-linear", "cubic-bezier(0, 0, 1, 1)");
    Object.defineProperty(window, "requestAnimationFrame", {
      configurable: true,
      value: (callback: FrameRequestCallback) => {
        callback(0);
        return 1;
      },
    });
    Object.defineProperty(document, "startViewTransition", {
      configurable: true,
      value: undefined,
    });
    const update = vi.fn();
    const changed = changeAppearanceFrom(
      trigger(),
      () => ({ ...MIDNIGHT, theme: "light", colorTheme: "ember" }),
      update,
    );

    await vi.waitFor(() => {
      expect(document.querySelector("[data-theme-transition-veil]")).not.toBeNull();
    });
    const veil = document.querySelector<HTMLElement>(
      "[data-theme-transition-veil]",
    );
    expect(veil?.dataset.phase).toBe("reveal");
    expect(veil?.dataset.colorScheme).toBe("light");
    expect(veil?.dataset.theme).toBe("ember");
    expect(update).not.toHaveBeenCalled();

    transitionEnd(veil!, "transform");
    await vi.waitFor(() => expect(veil?.dataset.phase).toBe("fade"));
    expect(update).toHaveBeenCalledOnce();
    expect(root.dataset.colorScheme).toBe("light");
    expect(root.dataset.theme).toBe("ember");

    transitionEnd(veil!, "opacity");
    await changed;
    expect(document.querySelector("[data-theme-transition-veil]")).toBeNull();
  });

  it("serializes native reveals and uses the composed 450ms motion", async () => {
    matchMedia(false);
    applyAppearance(MIDNIGHT);
    root.style.setProperty("--dur-theme", "300ms");
    root.style.setProperty("--dur-fast", "150ms");
    root.style.setProperty("--ease", "ease-out");
    root.style.setProperty("--ease-linear", "cubic-bezier(0, 0, 1, 1)");

    let releaseFirst = () => {};
    const firstFinished = new Promise<void>((resolve) => {
      releaseFirst = resolve;
    });
    const start = vi.fn((commit: () => void) => {
      commit();
      return {
        ready: Promise.resolve(),
        finished: start.mock.calls.length === 1
          ? firstFinished
          : Promise.resolve(),
      };
    });
    Object.defineProperty(document, "startViewTransition", {
      configurable: true,
      value: start,
    });
    const animate = vi.fn((
      _keyframes: Keyframe[] | PropertyIndexedKeyframes | null,
      _options?: number | KeyframeAnimationOptions,
    ) => ({ finished: Promise.resolve() } as unknown as Animation));
    Object.defineProperty(root, "animate", {
      configurable: true,
      value: animate,
    });

    const first = changeAppearanceFrom(
      trigger(),
      () => ({ ...MIDNIGHT, colorTheme: "aurora" }),
      vi.fn(),
    );
    const second = changeAppearanceFrom(
      trigger(),
      () => ({ ...MIDNIGHT, colorTheme: "ember" }),
      vi.fn(),
    );

    await vi.waitFor(() => expect(start).toHaveBeenCalledTimes(1));
    expect(animate).toHaveBeenCalledTimes(1);
    expect(animate.mock.calls[0]?.[1]).toMatchObject({
      duration: 450,
      easing: "cubic-bezier(0, 0, 1, 1)",
      pseudoElement: "::view-transition-new(root)",
    });

    releaseFirst();
    await first;
    await second;

    expect(start).toHaveBeenCalledTimes(2);
    expect(root.dataset.theme).toBe("ember");
    expect(root.dataset.themeTransition).toBeUndefined();
  });
});
