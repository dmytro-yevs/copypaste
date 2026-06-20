/**
 * W5: SettingsView skin-token controls (CopyPaste-kp6f)
 *
 * Verifies that SettingsView uses --skin-r-card / --skin-r-ctl CSS variable
 * references via inline styles instead of hardcoded Tailwind classes
 * (rounded-ide / rounded-ide-lg) for all card panels, status banners, and
 * interactive controls. Classic skin: --skin-r-ctl=9px / --skin-r-card=14px,
 * so the visual output is byte-identical for the default skin.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import { SettingsView } from "./SettingsView";
import { ErrorBoundary } from "../components/ErrorBoundary";

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

/** Checks that no element in the container has the given class. */
function noElementHasClass(container: HTMLElement, cls: string): boolean {
  return !Array.from(container.querySelectorAll("*")).some((el) =>
    el.classList.contains(cls),
  );
}

/** Checks that at least one element's inline style contains the given token. */
function someInlineStyleContains(container: HTMLElement, token: string): boolean {
  return Array.from(container.querySelectorAll<HTMLElement>("*")).some((el) =>
    el.style.cssText.includes(token) ||
    el.getAttribute("style")?.includes(token),
  );
}

// ---------------------------------------------------------------------------
// Mock: offline daemon (simplest render path — just need the DOM)
// ---------------------------------------------------------------------------

beforeEach(() => {
  invoke.mockReset();
  // Offline: all IPC calls reject. The component shows the offline banner.
  invoke.mockRejectedValue("daemon_offline:/tmp/copypaste.sock");
});

// ---------------------------------------------------------------------------
// §A  Panel() component — outer and inner wrappers
// ---------------------------------------------------------------------------

describe("§W5-A  SettingsView Panel — skin-driven card radius", () => {
  it("Panel outer wrapper does NOT use hardcoded rounded-ide-lg class", async () => {
    const { container } = render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );
    await waitFor(() =>
      expect(screen.getByText(/Daemon not running/i)).toBeInTheDocument(),
    );
    // rounded-ide-lg = hardcoded 14px; must be driven by --skin-r-card instead
    expect(noElementHasClass(container, "rounded-ide-lg")).toBe(true);
  });

  it("Panel outer wrapper uses var(--skin-r-card) in inline style", async () => {
    const { container } = render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );
    await waitFor(() =>
      expect(screen.getByText(/Daemon not running/i)).toBeInTheDocument(),
    );
    expect(someInlineStyleContains(container, "--skin-r-card")).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// §B  InfoPopover (~329) — popup bubble radius
// ---------------------------------------------------------------------------

describe("§W5-B  SettingsView InfoPopover — skin-driven control radius", () => {
  it("InfoPopover does NOT use hardcoded rounded-ide class for the bubble", async () => {
    const { container } = render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );
    await waitFor(() =>
      expect(screen.getByText(/Daemon not running/i)).toBeInTheDocument(),
    );
    // The popover bubble uses rounded-ide; it should be --skin-r-ctl instead.
    // Verify no element that is a popover (surface-glass-strong) has rounded-ide hardcoded.
    const popoverBubbles = container.querySelectorAll(".surface-glass-strong");
    for (const el of popoverBubbles) {
      expect(el.classList.contains("rounded-ide")).toBe(false);
    }
  });
});

// ---------------------------------------------------------------------------
// §C  Interactive controls — buttons / inputs (inline style --skin-r-ctl)
// ---------------------------------------------------------------------------

describe("§W5-C  SettingsView controls — skin-driven control radius", () => {
  it("does NOT contain any element with hardcoded rounded-ide class", async () => {
    const { container } = render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );
    await waitFor(() =>
      expect(screen.getByText(/Daemon not running/i)).toBeInTheDocument(),
    );
    expect(noElementHasClass(container, "rounded-ide")).toBe(true);
  });

  it("uses var(--skin-r-ctl) in at least one inline style (controls use skin token)", async () => {
    const { container } = render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );
    await waitFor(() =>
      expect(screen.getByText(/Daemon not running/i)).toBeInTheDocument(),
    );
    expect(someInlineStyleContains(container, "--skin-r-ctl")).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// §D  Status banners (notReady / offline / loading-error) — card radius
// ---------------------------------------------------------------------------

describe("§W5-D  SettingsView status banners — skin-driven card radius", () => {
  it("offline banner does NOT use hardcoded rounded-ide-lg class", async () => {
    const { container } = render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );
    await waitFor(() =>
      expect(screen.getByText(/Daemon not running/i)).toBeInTheDocument(),
    );
    // No element anywhere in the tree should have rounded-ide-lg (covers banners too).
    expect(noElementHasClass(container, "rounded-ide-lg")).toBe(true);
  });

  it("offline banner uses var(--skin-r-card) in inline style", async () => {
    const { container } = render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );
    await waitFor(() =>
      expect(screen.getByText(/Daemon not running/i)).toBeInTheDocument(),
    );
    // Find the banner div that is a direct wrapper (has surface-card class and
    // contains "Daemon not running" in its text content).
    const bannerDiv = Array.from(
      container.querySelectorAll<HTMLElement>("div"),
    ).find(
      (el) =>
        el.className.includes("surface-card") &&
        el.textContent?.includes("Daemon not running"),
    );
    expect(bannerDiv).not.toBeUndefined();
    const style = bannerDiv!.getAttribute("style") ?? "";
    expect(style).toContain("--skin-r-card");
  });
});

// ---------------------------------------------------------------------------
// §E  Outer shell card (~2734) — skin-driven card radius
// ---------------------------------------------------------------------------

describe("§W5-E  SettingsView outer shell card — skin-driven card radius", () => {
  it("outer shell card does NOT use hardcoded rounded-ide-lg class", async () => {
    // Need daemon online to render the tab shell card (hidden during loading).
    // We still use offline mock, but check that if the class were there it would fail.
    // The shell card is inside the `loadState !== "loading"` branch — with offline
    // the component renders it. The outer mx-auto > surface-card div is the shell.
    const { container } = render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );
    await waitFor(() =>
      expect(screen.getByText(/Daemon not running/i)).toBeInTheDocument(),
    );
    // No element anywhere should have rounded-ide-lg (we already check in §A,
    // but this is an explicit check for the shell card)
    expect(noElementHasClass(container, "rounded-ide-lg")).toBe(true);
  });
});
