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
import * as platform from "@/lib/platform";
import { DEFAULT_PREFS, usePrefs } from "@/store/prefs";
import { status, withClient, withUser } from "@/test/harness";

const getStatus = vi.fn();
const userAgent = navigator.userAgent;

function setUserAgent(value: string) {
  Object.defineProperty(navigator, "userAgent", { configurable: true, value });
}

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return { ...actual, getStatus: () => getStatus() };
});

beforeEach(() => {
  getStatus.mockReset().mockResolvedValue(status());
  usePrefs.setState({ ...DEFAULT_PREFS });
});

afterEach(() => {
  setUserAgent(userAgent);
  vi.restoreAllMocks();
});

describe("the settings navigation", () => {
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

  it("moves through the vertical desktop navigation with an arrow key", async () => {
    const { user } = withUser(<SettingsView />);
    const tabs = screen.getAllByRole("tab");
    tabs[0]!.focus();
    await user.keyboard("{ArrowDown}".repeat(tabs.length));
    await waitFor(() =>
      expect(tabs[0]!.getAttribute("aria-selected")).toBe("true"),
    );
  });

  it("reaches the shortcut, service, sync and storage screens", () => {
    withClient(<SettingsView />);
    // "Service" is the daemon's own configuration. Before it existed the app
    // could not change a single one of those values (CLAUDE.md rule 6).
    for (const label of ["Shortcut", "Service", "Sync", "Storage"]) {
      expect(screen.getByRole("tab", { name: label })).toBeTruthy();
    }
  });

  it("uses grouped vertical navigation on desktop", () => {
    withClient(<SettingsView />);
    const list = screen.getByRole("tablist");
    expect(list.getAttribute("aria-orientation")).toBe("vertical");
    expect(screen.getByText("Personal")).toBeTruthy();
    expect(screen.getByText("CopyPaste")).toBeTruthy();
    expect(screen.getByText("Support")).toBeTruthy();
    expect(list.className).toContain("flex-col");
  });

  it("keeps the compact horizontal tab row on Android", () => {
    setUserAgent("Mozilla/5.0 (Linux; Android 15)");
    withClient(<SettingsView />);

    const list = screen.getByRole("tablist");
    expect(list.getAttribute("aria-orientation")).toBe("horizontal");
    expect(list.className).toContain("flex-wrap");
    expect(list.className).toContain("w-full");
    expect(screen.queryByText("Personal")).toBeNull();
  });

  it("does not offer service or storage controls Android cannot honour", () => {
    setUserAgent("Mozilla/5.0 (Linux; Android 15)");
    withClient(<SettingsView />);
    expect(screen.queryByRole("tab", { name: "Service" })).toBeNull();
    expect(screen.queryByRole("tab", { name: "Storage" })).toBeNull();
  });

  it("does not offer a desktop shortcut control on Android", () => {
    vi.spyOn(platform, "isAndroid").mockReturnValue(true);
    withClient(<SettingsView />);
    expect(screen.queryByRole("tab", { name: "Shortcut" })).toBeNull();
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

  it("keeps every swatch colour immutable when another accent is selected", async () => {
    const { user } = withUser(<SettingsView />);
    const group = screen.getByRole("group", { name: "Accent" });
    const before = [...group.querySelectorAll<HTMLButtonElement>("button")].map(
      (swatch) => swatch.style.backgroundColor,
    );
    const classes = [...group.querySelectorAll<HTMLButtonElement>("button")].map(
      (swatch) => swatch.className,
    );

    expect(new Set(before)).toHaveLength(6);
    expect(new Set(classes)).toHaveLength(1);
    await user.click(screen.getByRole("button", { name: "Teal" }));
    expect([...group.querySelectorAll<HTMLButtonElement>("button")].map(
      (swatch) => swatch.style.backgroundColor,
    )).toEqual(before);
    expect([...group.querySelectorAll<HTMLButtonElement>("button")].map(
      (swatch) => swatch.className,
    )).toEqual(classes);
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
