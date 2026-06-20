/**
 * W-C1: Sidebar skin-token tests (CopyPaste-k7qy).
 *
 * Verifies that Sidebar renders skin-specific active-nav styles and that
 * structural dimensions (radius, shadow) are driven by --skin-* CSS vars
 * rather than hardcoded Tailwind tokens.
 *
 * Coverage matrix:
 *   classic — fill+glow active: nav-active-glow + accent gradient
 *   quiet   — tint active: accent-tint bg, no glow animation
 *   vapor   — glass+ring active: surface-glass border + ring, no fill gradient
 *
 * Structural token checks (radius/shadow use inline CSS vars, not hardcoded classes):
 *   - sidebar container: no rounded-ide-lg class; uses var(--skin-r-card) inline
 *   - sidebar container: no shadow-ide-sm class; uses var(--skin-shadow-card) inline
 *   - nav buttons: no rounded-ide class; uses var(--skin-r-ctl) inline
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render } from "@testing-library/react";
import { act } from "react";

// Mock Tauri so Sidebar can import without crashing in jsdom
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue({ ok: true, data: null }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("./SyncStatusChip", () => ({
  SyncStatusChip: () => <span data-testid="sync-status-chip" />,
}));

import { Sidebar } from "./Sidebar";
import { useUI } from "../store";

// Reset zustand store to default prefs before each test
beforeEach(() => {
  act(() => {
    useUI.setState({
      view: "history",
      prefs: {
        ...useUI.getState().prefs,
        skin: "classic",
      },
    });
  });
});

// Helper: set a skin pref and re-get state
function setSkin(skin: "classic" | "quiet" | "vapor") {
  act(() => {
    useUI.setState({
      prefs: { ...useUI.getState().prefs, skin },
    });
  });
}

// ---------------------------------------------------------------------------
// §W-C1-A  Classic skin — fill+glow active nav (feature-preservation)
// ---------------------------------------------------------------------------
describe("W-C1-A  classic skin — fill+glow active nav (unchanged from pre-skin)", () => {
  it("active nav item has nav-active-glow class in classic skin", () => {
    setSkin("classic");
    const { container } = render(<Sidebar />);
    const activeBtn = container.querySelector("button.nav-active-glow");
    expect(activeBtn).not.toBeNull();
  });

  it("active nav item has accent gradient classes in classic skin", () => {
    setSkin("classic");
    const { container } = render(<Sidebar />);
    const activeBtn = container.querySelector("button.nav-active-glow");
    expect(activeBtn).not.toBeNull();
    const hasGradient =
      activeBtn!.className.includes("bg-gradient") ||
      (activeBtn as HTMLElement).style.background?.includes("gradient");
    expect(hasGradient).toBe(true);
  });

  it("active nav item text is white in classic skin", () => {
    setSkin("classic");
    const { container } = render(<Sidebar />);
    const activeBtn = container.querySelector("button.nav-active-glow");
    expect(activeBtn!.className).toMatch(/text-white/);
  });
});

// ---------------------------------------------------------------------------
// §W-C1-B  Quiet skin — tint active nav
// ---------------------------------------------------------------------------
describe("W-C1-B  quiet skin — tint active nav", () => {
  it("active nav item has NO nav-active-glow in quiet skin", () => {
    setSkin("quiet");
    const { container } = render(<Sidebar />);
    const glowBtn = container.querySelector("button.nav-active-glow");
    expect(glowBtn).toBeNull();
  });

  it("active nav item has NO accent-gradient fill in quiet skin", () => {
    setSkin("quiet");
    const { container } = render(<Sidebar />);
    // Find the active button (History is default view)
    const buttons = Array.from(container.querySelectorAll("nav button"));
    const activeBtn = buttons.find((btn) =>
      btn.textContent?.includes("History")
    ) as HTMLElement | undefined;
    expect(activeBtn).toBeDefined();
    expect(activeBtn!.className).not.toMatch(/bg-gradient/);
  });

  it("active nav item has data-skin-active or accent tint styling in quiet skin", () => {
    setSkin("quiet");
    const { container } = render(<Sidebar />);
    const buttons = Array.from(container.querySelectorAll("nav button"));
    const activeBtn = buttons.find((btn) =>
      btn.textContent?.includes("History")
    ) as HTMLElement | undefined;
    expect(activeBtn).toBeDefined();
    // Should have accent-tint class or style indicating active state without glow
    const cls = activeBtn!.className;
    const style = activeBtn!.getAttribute("style") ?? "";
    // Must have some active indicator that isn't the fill+glow pattern
    const hasActiveIndicator =
      cls.includes("bg-ide-accent") ||
      cls.includes("ide-accentDim") ||
      style.includes("background") ||
      cls.includes("skin-active") ||
      activeBtn!.hasAttribute("data-skin-active");
    expect(hasActiveIndicator).toBe(true);
  });

  it("inactive nav items still have ide-dim text in quiet skin", () => {
    setSkin("quiet");
    const { container } = render(<Sidebar />);
    const buttons = Array.from(container.querySelectorAll("nav button")).filter(
      (btn) => !btn.textContent?.includes("History")
    );
    expect(buttons.length).toBeGreaterThanOrEqual(4);
    for (const btn of buttons) {
      expect(btn.className).toMatch(/text-ide-dim/);
    }
  });
});

// ---------------------------------------------------------------------------
// §W-C1-C  Vapor skin — glass+ring active nav
// ---------------------------------------------------------------------------
describe("W-C1-C  vapor skin — glass+ring active nav", () => {
  it("active nav item has NO nav-active-glow in vapor skin", () => {
    setSkin("vapor");
    const { container } = render(<Sidebar />);
    const glowBtn = container.querySelector("button.nav-active-glow");
    expect(glowBtn).toBeNull();
  });

  it("active nav item has NO accent gradient fill in vapor skin", () => {
    setSkin("vapor");
    const { container } = render(<Sidebar />);
    const buttons = Array.from(container.querySelectorAll("nav button"));
    const activeBtn = buttons.find((btn) =>
      btn.textContent?.includes("History")
    ) as HTMLElement | undefined;
    expect(activeBtn).toBeDefined();
    expect(activeBtn!.className).not.toMatch(/bg-gradient/);
  });

  it("active nav item has ring or glass styling in vapor skin", () => {
    setSkin("vapor");
    const { container } = render(<Sidebar />);
    const buttons = Array.from(container.querySelectorAll("nav button"));
    const activeBtn = buttons.find((btn) =>
      btn.textContent?.includes("History")
    ) as HTMLElement | undefined;
    expect(activeBtn).toBeDefined();
    const cls = activeBtn!.className;
    const style = activeBtn!.getAttribute("style") ?? "";
    // Vapor active: ring or glass surface or outline indicator
    const hasRingOrGlass =
      cls.includes("ring") ||
      cls.includes("surface-glass") ||
      cls.includes("outline") ||
      style.includes("ring") ||
      style.includes("box-shadow") ||
      style.includes("outline");
    expect(hasRingOrGlass).toBe(true);
  });

  it("inactive nav items still have ide-dim text in vapor skin", () => {
    setSkin("vapor");
    const { container } = render(<Sidebar />);
    const buttons = Array.from(container.querySelectorAll("nav button")).filter(
      (btn) => !btn.textContent?.includes("History")
    );
    expect(buttons.length).toBeGreaterThanOrEqual(4);
    for (const btn of buttons) {
      expect(btn.className).toMatch(/text-ide-dim/);
    }
  });
});

// ---------------------------------------------------------------------------
// §W-C1-D  Structural token checks — radius and shadow via --skin-* vars
// ---------------------------------------------------------------------------
describe("W-C1-D  Sidebar structural tokens — skin-driven radius + shadow", () => {
  it("sidebar <aside> does NOT use hardcoded rounded-ide-lg class", () => {
    const { container } = render(<Sidebar />);
    const aside = container.querySelector("aside");
    expect(aside).not.toBeNull();
    // rounded-ide-lg is hardcoded 14px; should be replaced with --skin-r-card inline
    expect(aside!.className).not.toMatch(/\brounded-ide-lg\b/);
  });

  it("sidebar <aside> uses var(--skin-r-card) for border-radius", () => {
    const { container } = render(<Sidebar />);
    const aside = container.querySelector("aside") as HTMLElement;
    // Either inline style borderRadius references --skin-r-card or
    // a CSS class that wraps it (surface-glass-card or similar)
    const style = aside.getAttribute("style") ?? "";
    expect(style).toMatch(/--skin-r-card/);
  });

  it("sidebar <aside> does NOT use shadow-ide-sm class (hardcoded e2)", () => {
    const { container } = render(<Sidebar />);
    const aside = container.querySelector("aside");
    expect(aside!.className).not.toMatch(/\bshadow-ide-sm\b/);
  });

  it("sidebar <aside> uses var(--skin-shadow-card) for box-shadow", () => {
    const { container } = render(<Sidebar />);
    const aside = container.querySelector("aside") as HTMLElement;
    const style = aside.getAttribute("style") ?? "";
    expect(style).toMatch(/--skin-shadow-card/);
  });

  it("nav buttons do NOT use hardcoded rounded-ide class", () => {
    const { container } = render(<Sidebar />);
    const buttons = container.querySelectorAll("nav button");
    for (const btn of buttons) {
      expect(btn.className).not.toMatch(/\brounded-ide\b/);
    }
  });

  it("nav buttons use var(--skin-r-ctl) for border-radius via inline style", () => {
    const { container } = render(<Sidebar />);
    const buttons = Array.from(container.querySelectorAll("nav button"));
    for (const btn of buttons) {
      const style = (btn as HTMLElement).getAttribute("style") ?? "";
      expect(style).toMatch(/--skin-r-ctl/);
    }
  });
});

// ---------------------------------------------------------------------------
// §W-C1-E  Feature preservation — all existing behaviours survive skin switch
// ---------------------------------------------------------------------------
describe("W-C1-E  feature preservation across all skins", () => {
  for (const skin of ["classic", "quiet", "vapor"] as const) {
    describe(`skin: ${skin}`, () => {
      it("renders <aside> with surface-glass class", () => {
        setSkin(skin);
        const { container } = render(<Sidebar />);
        expect(container.querySelector("aside.surface-glass")).not.toBeNull();
      });

      it("renders 5 nav items", () => {
        setSkin(skin);
        const { container } = render(<Sidebar />);
        const buttons = container.querySelectorAll("nav button");
        expect(buttons.length).toBe(5);
      });

      it("renders accent-tint radial overlay", () => {
        setSkin(skin);
        const { container } = render(<Sidebar />);
        expect(container.querySelector("[data-accent-tint]")).not.toBeNull();
      });

      it("has drag region", () => {
        setSkin(skin);
        const { container } = render(<Sidebar />);
        expect(
          container.querySelector("[data-tauri-drag-region]")
        ).not.toBeNull();
      });

      it("nav buttons all have list-item-in entrance", () => {
        setSkin(skin);
        const { container } = render(<Sidebar />);
        for (const btn of container.querySelectorAll("nav button")) {
          expect(btn.className).toMatch(/list-item-in/);
        }
      });

      it("no hardcoded hex colours in class names", () => {
        setSkin(skin);
        const { container } = render(<Sidebar />);
        const classes = Array.from(container.querySelectorAll("*"))
          .map((el) => el.className)
          .filter((c) => typeof c === "string")
          .join(" ");
        expect(classes).not.toMatch(/#[0-9a-fA-F]{3,6}/);
      });
    });
  }
});
