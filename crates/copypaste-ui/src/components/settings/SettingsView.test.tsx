/**
 * Settings: the tab contract (A11Y-6 / AT-18), the reflow rule (A11Y-15 /
 * AT-19), and the two controls whose *copy* was a bug fix (CopyPaste-8ebg.63).
 *
 * The tab behaviour comes from Radix, so these tests are checking that we wired
 * it rather than that it works — which is the point of using it: manifest §9.1
 * names v1's `tabListKeyDown` factory as something to delete.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { SettingsView } from "@/components/settings/SettingsView";
import { DEFAULT_PREFS, usePrefs } from "@/store/prefs";
import { status, withClient, withUser } from "@/test/harness";

const getStatus = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return { ...actual, getStatus: () => getStatus() };
});

beforeEach(() => {
  getStatus.mockReset().mockResolvedValue(status());
  usePrefs.setState({ ...DEFAULT_PREFS });
});

afterEach(() => vi.restoreAllMocks());

describe("the tab row (A11Y-6 / AT-18)", () => {
  it("is a tablist of tabs with a selected one", () => {
    withClient(<SettingsView />);
    const tabs = screen.getAllByRole("tab");
    expect(tabs.length).toBeGreaterThanOrEqual(6);
    expect(tabs.filter((tab) => tab.getAttribute("aria-selected") === "true")).toHaveLength(1);
    expect(screen.getByRole("tablist", { name: "Settings sections" })).toBeTruthy();
  });

  it("pairs each pane with its tab", async () => {
    withClient(<SettingsView />);
    const panel = screen.getByRole("tabpanel");
    const labelledBy = panel.getAttribute("aria-labelledby");
    expect(labelledBy).toBeTruthy();
    expect(document.getElementById(labelledBy!)?.getAttribute("role")).toBe("tab");
  });

  it("wraps from the last tab back to the first with an arrow key", async () => {
    const { user } = withUser(<SettingsView />);
    const tabs = screen.getAllByRole("tab");
    tabs[0]!.focus();
    // One right per tab lands back where it started, which is the wrap.
    await user.keyboard("{ArrowRight}".repeat(tabs.length));
    await waitFor(() =>
      expect(tabs[0]!.getAttribute("aria-selected")).toBe("true"),
    );
  });

  it("reaches the shortcut, sync and storage screens", () => {
    withClient(<SettingsView />);
    for (const label of ["Shortcut", "Sync", "Storage"]) {
      expect(screen.getByRole("tab", { name: label })).toBeTruthy();
    }
  });

  it("wraps rather than scrolling, so nothing hides at 720px (A11Y-15)", () => {
    // CopyPaste-g27b.31: at the minimum width v1's tab row overflowed behind a
    // scrollbar-less scroller and its last tab was entirely off-screen. jsdom
    // cannot measure that, so the invariant is asserted where it lives.
    withClient(<SettingsView />);
    const list = screen.getByRole("tablist");
    expect(list.className).toContain("flex-wrap");
    expect(list.className).not.toContain("overflow-x");
  });
});

describe("appearance", () => {
  it("says what System currently resolves to (CopyPaste-8ebg.63)", () => {
    withClient(<SettingsView />);
    expect(screen.getByText(/Currently resolves to (Dark|Light)\./)).toBeTruthy();
  });

  it("exposes each accent as a named, pressable control (A11Y-8, A11Y-9)", () => {
    withClient(<SettingsView />);
    const group = screen.getByRole("group", { name: "Accent" });
    const swatches = group.querySelectorAll("button");
    expect(swatches).toHaveLength(6);
    for (const swatch of swatches) {
      expect(swatch.getAttribute("aria-label")?.length ?? 0).toBeGreaterThan(0);
      expect(swatch.getAttribute("aria-pressed")).toMatch(/true|false/);
    }
    expect(group.querySelectorAll('[aria-pressed="true"]')).toHaveLength(1);
  });

  it("persists a change through the store", async () => {
    const { user } = withUser(<SettingsView />);
    await user.click(screen.getByRole("button", { name: "Teal" }));
    expect(usePrefs.getState().accent).toBe("teal");
  });
});

describe("list settings", () => {
  it("formats the preview-line value with its unit", async () => {
    const { user } = withUser(<SettingsView />);
    await user.click(screen.getByRole("tab", { name: "List" }));
    // A bare number was meaningless (CopyPaste-8ebg.63).
    expect(await screen.findByText("2 lines")).toBeTruthy();
  });

  it("gives the slider an accessible name", async () => {
    const { user } = withUser(<SettingsView />);
    await user.click(screen.getByRole("tab", { name: "List" }));
    expect(await screen.findByRole("slider", { name: "Preview lines" })).toBeTruthy();
  });
});

describe("about", () => {
  it("reports the backend, and flags one that is not the real clipboard", async () => {
    getStatus.mockResolvedValue(status({ clipboard_backend: "fake" }));
    const { user } = withUser(<SettingsView />);
    await user.click(screen.getByRole("tab", { name: "About" }));
    expect(await screen.findByText("fake")).toBeTruthy();
  });

  it("names no path anywhere", async () => {
    const { user, container } = withUser(<SettingsView />);
    await user.click(screen.getByRole("tab", { name: "About" }));
    await screen.findByText(/CopyPaste 2\./);
    expect(container.innerHTML).not.toMatch(/\/Users\/|\/home\/|\.sock/);
  });
});
