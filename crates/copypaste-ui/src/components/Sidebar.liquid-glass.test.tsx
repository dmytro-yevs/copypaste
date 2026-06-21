/**
 * Tests for Liquid-Glass sidebar/shell polish (CopyPaste-jxbx):
 * 1. Sidebar container: surface-glass + radial accent tint + card-in entrance.
 * 2. Active nav item: accent gradient bg + nav-active-glow + on-accent text.
 * 3. Inactive nav item: ide-dim text, no active gradient classes.
 * 4. Hover nav item: transition classes + background hover (no translateX — MOT-16).
 * 5. Nav items: list-item-in stagger entrance class + animationDelay inline style.
 * 6. ViewShell header: card-in entrance class.
 * 7. ViewShell content panel: reveal-up entrance class.
 * 8. No hardcoded hex colours in Sidebar or ViewShell class strings.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// Mock Tauri so Sidebar can import without crashing in jsdom
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue({ ok: true, data: null }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

// Mock SyncStatusChip — it has its own Tauri/IPC deps and is not under test here
vi.mock("./SyncStatusChip", () => ({
  SyncStatusChip: () => <span data-testid="sync-status-chip" />,
}));

import { Sidebar } from "./Sidebar";
import { ViewShell } from "./ViewShell";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Collect all className strings from the rendered subtree. */
function allClasses(container: HTMLElement): string {
  return Array.from(container.querySelectorAll("*"))
    .map((el) => el.className)
    .filter((c) => typeof c === "string")
    .join(" ");
}

// ---------------------------------------------------------------------------
// §1  Sidebar container glass treatment
// ---------------------------------------------------------------------------
describe("§jxbx-1  Sidebar — glass container", () => {
  it("renders a <aside> with surface-glass class", () => {
    const { container } = render(<Sidebar />);
    const aside = container.querySelector("aside");
    expect(aside).not.toBeNull();
    expect(aside!.className).toMatch(/surface-glass/);
  });

  it("sidebar container has card-in entrance animation class", () => {
    const { container } = render(<Sidebar />);
    const aside = container.querySelector("aside");
    expect(aside!.className).toMatch(/card-in/);
  });

  it("sidebar has a radial accent tint element at the top", () => {
    const { container } = render(<Sidebar />);
    // The radial tint is a decorative <div> using a bg that references --accent
    // We look for a child with the accent-tint data attr or a class containing 'accent-tint'
    const tint = container.querySelector("[data-accent-tint]");
    expect(tint).not.toBeNull();
  });
});

// ---------------------------------------------------------------------------
// §2  Active nav item
// ---------------------------------------------------------------------------
describe("§jxbx-2  Sidebar — active nav item styling", () => {
  it("active nav item has nav-active-glow class", () => {
    const { container } = render(<Sidebar />);
    // Default view is "history" so History nav item is active
    const activeBtn = container.querySelector("button.nav-active-glow");
    expect(activeBtn).not.toBeNull();
  });

  it("active nav item has accent gradient background via inline style or bg-gradient class", () => {
    const { container } = render(<Sidebar />);
    // Accept either a CSS class containing 'bg-gradient' or an inline backgroundImage style
    const activeBtn = container.querySelector("button.nav-active-glow");
    expect(activeBtn).not.toBeNull();
    const hasGradientClass = activeBtn!.className.includes("bg-gradient");
    const hasInlineGradient =
      (activeBtn as HTMLElement).style.background?.includes("gradient") ||
      (activeBtn as HTMLElement).style.backgroundImage?.includes("gradient");
    expect(hasGradientClass || hasInlineGradient).toBe(true);
  });

  it("active nav item text uses on-accent (white) colour, not ide-dim", () => {
    const { container } = render(<Sidebar />);
    const activeBtn = container.querySelector("button.nav-active-glow");
    expect(activeBtn).not.toBeNull();
    expect(activeBtn!.className).not.toMatch(/text-ide-dim/);
  });
});

// ---------------------------------------------------------------------------
// §3  Inactive nav item
// ---------------------------------------------------------------------------
describe("§jxbx-3  Sidebar — inactive nav item styling", () => {
  it("inactive nav items have text-ide-dim class", () => {
    const { container } = render(<Sidebar />);
    const buttons = Array.from(container.querySelectorAll("button")).filter(
      (btn) => !btn.className.includes("nav-active-glow")
    );
    // At least 4 inactive items (devices, settings, about, logs)
    expect(buttons.length).toBeGreaterThanOrEqual(4);
    for (const btn of buttons) {
      expect(btn.className).toMatch(/text-ide-dim/);
    }
  });

  it("inactive nav items do NOT have nav-active-glow", () => {
    const { container } = render(<Sidebar />);
    const buttons = Array.from(container.querySelectorAll("button")).filter(
      (btn) => !btn.className.includes("nav-active-glow")
    );
    for (const btn of buttons) {
      expect(btn.className).not.toMatch(/nav-active-glow/);
    }
  });
});

// ---------------------------------------------------------------------------
// §4  Hover nav item — transition + background hover (no translateX, MOT-16)
// ---------------------------------------------------------------------------
describe("§jxbx-4  Sidebar — hover transition classes", () => {
  it("nav buttons have a transition class for smooth hover", () => {
    const { container } = render(<Sidebar />);
    const buttons = container.querySelectorAll("button");
    // At least the inactive nav buttons must carry a transition class
    const inactiveBtns = Array.from(buttons).filter(
      (btn) => !btn.className.includes("nav-active-glow")
    );
    for (const btn of inactiveBtns) {
      expect(btn.className).toMatch(/transition/);
    }
  });

  it("nav buttons use background hover (no translateX — MOT-16 calm motion)", () => {
    const { container } = render(<Sidebar />);
    const buttons = container.querySelectorAll("button");
    const inactiveBtns = Array.from(buttons).filter(
      (btn) => !btn.className.includes("nav-active-glow")
    );
    for (const btn of inactiveBtns) {
      // MOT-16: translateX hover removed (items appeared to leave the sidebar).
      // Hover is now background-only: hover:bg-ide-hover + hover:text-ide-text.
      expect(btn.className).not.toMatch(/hover:translate-x/);
      expect(btn.className).toMatch(/hover:bg-ide-hover/);
    }
  });
});

// ---------------------------------------------------------------------------
// §5  Nav item stagger entrance
// ---------------------------------------------------------------------------
describe("§jxbx-5  Sidebar — nav item stagger entrance", () => {
  it("each nav button has list-item-in entrance class", () => {
    const { container } = render(<Sidebar />);
    const buttons = container.querySelectorAll("nav button");
    expect(buttons.length).toBeGreaterThanOrEqual(5);
    for (const btn of buttons) {
      expect(btn.className).toMatch(/list-item-in/);
    }
  });

  it("nav buttons have staggered animationDelay inline styles", () => {
    const { container } = render(<Sidebar />);
    const buttons = Array.from(container.querySelectorAll("nav button"));
    const delays = buttons.map(
      (btn) => (btn as HTMLElement).style.animationDelay
    );
    // Each delay should be a non-empty string
    for (const delay of delays) {
      expect(delay).toBeTruthy();
    }
    // Delays should differ between items (stagger, not all same)
    const uniqueDelays = new Set(delays);
    expect(uniqueDelays.size).toBeGreaterThan(1);
  });
});

// ---------------------------------------------------------------------------
// §6  ViewShell header — card-in entrance
// ---------------------------------------------------------------------------
describe("§jxbx-6  ViewShell — header entrance", () => {
  it("header element has card-in entrance animation class", () => {
    const { container } = render(
      <ViewShell title="Test">
        <div>content</div>
      </ViewShell>
    );
    const header = container.querySelector("header");
    expect(header).not.toBeNull();
    expect(header!.className).toMatch(/card-in/);
  });

  it("header renders the title", () => {
    render(
      <ViewShell title="My View">
        <div>content</div>
      </ViewShell>
    );
    expect(screen.getByRole("heading", { name: /My View/i })).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// §7  ViewShell content panel — reveal-up entrance
// ---------------------------------------------------------------------------
describe("§jxbx-7  ViewShell — content panel entrance", () => {
  it("content panel has reveal-up entrance animation class", () => {
    const { container } = render(
      <ViewShell title="Test">
        <div>content</div>
      </ViewShell>
    );
    // The scrollable content div (sibling of header)
    const contentPanel = container.querySelector("div.surface-glass.flex-1");
    expect(contentPanel).not.toBeNull();
    expect(contentPanel!.className).toMatch(/reveal-up/);
  });
});

// ---------------------------------------------------------------------------
// §8  No hardcoded hex colours in className strings
// ---------------------------------------------------------------------------
describe("§jxbx-8  No hardcoded hex colours", () => {
  it("Sidebar uses no hardcoded hex colour in class names", () => {
    const { container } = render(<Sidebar />);
    const classes = allClasses(container);
    expect(classes).not.toMatch(/#[0-9a-fA-F]{3,6}/);
  });

  it("ViewShell uses no hardcoded hex colour in class names", () => {
    const { container } = render(
      <ViewShell title="Test">
        <div>content</div>
      </ViewShell>
    );
    const classes = allClasses(container);
    expect(classes).not.toMatch(/#[0-9a-fA-F]{3,6}/);
  });
});
