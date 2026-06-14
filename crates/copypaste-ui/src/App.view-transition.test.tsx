/**
 * Tests for smooth view-switch crossfade (CopyPaste-2bhh):
 *
 * 1. The view wrapper exposes data-testid="view-transition" on the <main> content.
 * 2. The wrapper carries a CSS transition class (view-fade-in or similar).
 * 3. prefers-reduced-motion: when the media query matches, no transition class.
 * 4. ViewShell: when rendered inside a view-transition wrapper, card-in /
 *    reveal-up should NOT be replaced by the parent animation — they coexist but
 *    the wrapper fade is the dominant motion on tab switch.
 * 5. The wrapper has an `animation-fill-mode: forwards` or equivalent so it
 *    doesn't flash-back after the animation ends.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render } from "@testing-library/react";
import { ViewShell } from "./components/ViewShell";

// ---------------------------------------------------------------------------
// 1–3: ViewTransitionWrapper (exported from App.tsx or its own module)
// ---------------------------------------------------------------------------

// We import the named export ViewTransitionWrapper from App.tsx
// (the implementation will export it alongside the default App export).
import { ViewTransitionWrapper } from "./App";

describe("§2bhh-1  ViewTransitionWrapper — rendered structure", () => {
  it("renders children", () => {
    const { getByText } = render(
      <ViewTransitionWrapper viewKey="history">
        <div>Hello</div>
      </ViewTransitionWrapper>
    );
    expect(getByText("Hello")).toBeInTheDocument();
  });

  it("root element has data-testid='view-transition'", () => {
    const { container } = render(
      <ViewTransitionWrapper viewKey="history">
        <div>content</div>
      </ViewTransitionWrapper>
    );
    expect(container.querySelector("[data-testid='view-transition']")).not.toBeNull();
  });

  it("root element has the view-fade-in CSS class for the animation", () => {
    const { container } = render(
      <ViewTransitionWrapper viewKey="history">
        <div>content</div>
      </ViewTransitionWrapper>
    );
    const wrapper = container.querySelector("[data-testid='view-transition']") as HTMLElement;
    expect(wrapper).not.toBeNull();
    expect(wrapper.className).toMatch(/view-fade-in/);
  });

  it("root element is full-height (h-full class or style)", () => {
    const { container } = render(
      <ViewTransitionWrapper viewKey="settings">
        <div>content</div>
      </ViewTransitionWrapper>
    );
    const wrapper = container.querySelector("[data-testid='view-transition']") as HTMLElement;
    expect(wrapper).not.toBeNull();
    // Must fill the parent <main> — either h-full tailwind class or style
    const hasHFull = wrapper.className.includes("h-full");
    const hasHeightStyle = wrapper.style.height === "100%";
    expect(hasHFull || hasHeightStyle).toBe(true);
  });
});

describe("§2bhh-2  ViewTransitionWrapper — animation properties", () => {
  it("animation-fill-mode is 'forwards' (no flash-back after fade ends)", () => {
    const { container } = render(
      <ViewTransitionWrapper viewKey="history">
        <div>content</div>
      </ViewTransitionWrapper>
    );
    const wrapper = container.querySelector("[data-testid='view-transition']") as HTMLElement;
    expect(wrapper).not.toBeNull();
    // Either via inline style or a CSS class that carries animation-fill-mode: forwards.
    // We check the inline style because jsdom doesn't run stylesheets.
    // The component must set animationFillMode: "forwards" via inline style.
    expect(wrapper.style.animationFillMode).toBe("forwards");
  });

  it("animation duration is set inline (150–220ms range for Apple-like feel)", () => {
    const { container } = render(
      <ViewTransitionWrapper viewKey="history">
        <div>content</div>
      </ViewTransitionWrapper>
    );
    const wrapper = container.querySelector("[data-testid='view-transition']") as HTMLElement;
    expect(wrapper).not.toBeNull();
    // animationDuration must be a CSS time value like "180ms" or "0.18s"
    const duration = wrapper.style.animationDuration;
    expect(duration).toBeTruthy();
    // Extract the numeric ms value — accept either "Xms" or "X.Xs" format
    const ms = duration.endsWith("ms")
      ? parseFloat(duration)
      : parseFloat(duration) * 1000;
    expect(ms).toBeGreaterThanOrEqual(150);
    expect(ms).toBeLessThanOrEqual(220);
  });
});

describe("§2bhh-3  ViewTransitionWrapper — prefers-reduced-motion", () => {
  let originalMatchMedia: typeof window.matchMedia;

  beforeEach(() => {
    originalMatchMedia = window.matchMedia;
  });

  afterEach(() => {
    window.matchMedia = originalMatchMedia;
  });

  it("skips animation when prefers-reduced-motion: reduce", () => {
    // Stub matchMedia to report reduced-motion preference
    window.matchMedia = vi.fn().mockImplementation((query: string) => ({
      matches: query === "(prefers-reduced-motion: reduce)",
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }));

    const { container } = render(
      <ViewTransitionWrapper viewKey="history">
        <div>content</div>
      </ViewTransitionWrapper>
    );
    const wrapper = container.querySelector("[data-testid='view-transition']") as HTMLElement;
    expect(wrapper).not.toBeNull();
    // When reduced motion is active, no animation class or animation is set
    const hasAnimation = wrapper.className.includes("view-fade-in") &&
      (wrapper.style.animationName !== "" || wrapper.style.animationDuration !== "");
    // The animation should be suppressed (either no class, or duration = 0, or animationName = none)
    const isDisabled =
      !wrapper.className.includes("view-fade-in") ||
      wrapper.style.animationDuration === "0ms" ||
      wrapper.style.animationDuration === "0s" ||
      wrapper.style.animationName === "none";
    expect(isDisabled || !hasAnimation).toBe(true);
  });

  it("shows animation when prefers-reduced-motion: no-preference", () => {
    window.matchMedia = vi.fn().mockImplementation((query: string) => ({
      matches: query !== "(prefers-reduced-motion: reduce)",
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }));

    const { container } = render(
      <ViewTransitionWrapper viewKey="history">
        <div>content</div>
      </ViewTransitionWrapper>
    );
    const wrapper = container.querySelector("[data-testid='view-transition']") as HTMLElement;
    expect(wrapper).not.toBeNull();
    expect(wrapper.className).toMatch(/view-fade-in/);
  });
});

describe("§2bhh-4  ViewShell — entrance classes preserved for first mount", () => {
  it("ViewShell header still has card-in class (entrance on mount)", () => {
    const { container } = render(
      <ViewShell title="History">
        <div>view content</div>
      </ViewShell>
    );
    const header = container.querySelector("header");
    expect(header).not.toBeNull();
    expect(header!.className).toMatch(/card-in/);
  });

  it("ViewShell content panel still has reveal-up class", () => {
    const { container } = render(
      <ViewShell title="History">
        <div>view content</div>
      </ViewShell>
    );
    const panel = container.querySelector("div.surface-glass.flex-1");
    expect(panel).not.toBeNull();
    expect(panel!.className).toMatch(/reveal-up/);
  });
});
