/**
 * Phase 4: Sidebar styling — §9.11 two-axis design system.
 * Active nav: bg-ide-selection + text-ide-text.
 * Inactive nav: text-ide-dim + hover:bg-ide-hover.
 * Removed: nav-active-glow, skin-specific branches.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

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
import { ViewShell } from "./ViewShell";

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
    const tint = container.querySelector("[data-accent-tint]");
    expect(tint).not.toBeNull();
  });
});

// ---------------------------------------------------------------------------
// §2  Active nav item — §9.11 two-axis: bg-ide-selection text-ide-text
// ---------------------------------------------------------------------------
describe("§jxbx-2  Sidebar — active nav item styling", () => {
  it("active nav item has bg-ide-selection class", () => {
    const { container } = render(<Sidebar />);
    // Default view is "history" so History nav item is active
    const activeBtn = container.querySelector("button.bg-ide-selection");
    expect(activeBtn).not.toBeNull();
  });

  it("active nav item has text-ide-text class", () => {
    const { container } = render(<Sidebar />);
    const activeBtn = container.querySelector("button.bg-ide-selection");
    expect(activeBtn).not.toBeNull();
    expect(activeBtn!.className).toMatch(/text-ide-text/);
  });

  it("active nav item does NOT have text-ide-dim class", () => {
    const { container } = render(<Sidebar />);
    const activeBtn = container.querySelector("button.bg-ide-selection");
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
      (btn) => !btn.className.includes("bg-ide-selection")
    );
    // At least 4 inactive items (devices, settings, about, logs)
    expect(buttons.length).toBeGreaterThanOrEqual(4);
    for (const btn of buttons) {
      expect(btn.className).toMatch(/text-ide-dim/);
    }
  });

  it("inactive nav items do NOT have bg-ide-selection", () => {
    const { container } = render(<Sidebar />);
    const buttons = Array.from(container.querySelectorAll("button")).filter(
      (btn) => !btn.className.includes("bg-ide-selection")
    );
    for (const btn of buttons) {
      expect(btn.className).not.toMatch(/bg-ide-selection/);
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
    for (const btn of Array.from(buttons)) {
      expect(btn.className).toMatch(/transition/);
    }
  });

  it("nav buttons use background hover (no translateX — MOT-16 calm motion)", () => {
    const { container } = render(<Sidebar />);
    const inactiveBtns = Array.from(container.querySelectorAll("button")).filter(
      (btn) => !btn.className.includes("bg-ide-selection")
    );
    for (const btn of inactiveBtns) {
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
    const delays = buttons.map((btn) => (btn as HTMLElement).style.animationDelay);
    for (const delay of delays) {
      expect(delay).toBeTruthy();
    }
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
