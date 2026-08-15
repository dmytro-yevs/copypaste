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
import { ACCENTS, DEFAULT_PREFS, usePrefs } from "@/store/prefs";
import { useUi } from "@/store/ui";
import { status, withClient, withUser } from "@/test/harness";
import { resetViewportWidth, setViewportWidth } from "@/test/viewport";

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
  useUi.setState({ view: "history", settingsTab: null });
});

afterEach(() => {
  setUserAgent(userAgent);
  resetViewportWidth();
  vi.restoreAllMocks();
});

describe("the settings navigation", () => {
  it("is a tablist of tabs with a selected one", () => {
    withClient(<SettingsView />);
    const tabs = screen.getAllByRole("tab");
    expect(tabs.length).toBeGreaterThanOrEqual(6);
    expect(tabs.filter((tab) => tab.getAttribute("aria-selected") === "true")).toHaveLength(1);
    expect(screen.getByRole("tablist", { name: "Settings sections" })).toBeTruthy();
    const search = screen.getByRole("searchbox", { name: "Search settings" })
      .closest<HTMLElement>('[role="search"]')!;
    expect(search.className).toContain("w-full");
    expect(search.className).toContain("relative");
    expect(search.className).not.toContain("max-w");

    const header = search.closest<HTMLElement>("header")!;
    expect(header.className).toContain("chrome");
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
    // could not change a single one of those values (AGENTS.md rule 6).
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
    expect(list.className).toContain("self-stretch");
    expect(list.className).toContain("border-r");
    expect(list.className).not.toContain("h-fit");
  });

  it("keeps the grouped two-pane navigation in a wide Android window", () => {
    setUserAgent("Mozilla/5.0 (Linux; Android 15)");
    setViewportWidth(891);
    withClient(<SettingsView />);

    const list = screen.getByRole("tablist");
    expect(list.getAttribute("aria-orientation")).toBe("vertical");
    expect(screen.getByText("Personal")).toBeTruthy();
    // The one tab that is Android's alone still belongs to a group rather than
    // to a flat strip.
    expect(screen.getByRole("tab", { name: "Background capture" })).toBeTruthy();
  });

  it("keeps the first native Storage target platform-visible on Android", async () => {
    setUserAgent("Mozilla/5.0 (Linux; Android 15)");
    const { user } = withUser(<SettingsView />);
    expect(screen.getByRole("tab", { name: "Service" })).toBeTruthy();
    const storage = screen.getByRole("tab", { name: "Storage" });

    await user.click(storage);

    expect(storage.getAttribute("aria-selected")).toBe("true");
    for (const label of ["Export…", "Import…", "Clear history"]) {
      expect(await screen.findByRole("button", { name: label })).toBeTruthy();
    }
  });

  it("does not offer a desktop shortcut control on Android", () => {
    vi.spyOn(platform, "isAndroid").mockReturnValue(true);
    withClient(<SettingsView />);
    expect(screen.queryByRole("tab", { name: "Shortcut" })).toBeNull();
  });

  it("does not offer desktop window translucency on Android", () => {
    setUserAgent("Mozilla/5.0 (Linux; Android 15)");
    withClient(<SettingsView />);
    expect(screen.queryByRole("slider", { name: "Translucency" })).toBeNull();
  });

  it("shows a compact search dropdown without replacing the current settings pane", async () => {
    const { user } = withUser(<SettingsView />);
    await user.type(screen.getByRole("searchbox", { name: "Search settings" }), "storage quota");

    expect(await screen.findByText("Storage quota")).toBeTruthy();
    expect(screen.getByText("Service · What gets kept")).toBeTruthy();
    expect(screen.getByRole("tablist", { name: "Settings sections" })).toBeTruthy();
    const dropdown = screen.getByRole("region", { name: "Settings search results" });
    const positioner = dropdown.parentElement!;
    expect(positioner.hasAttribute("data-radix-popper-content-wrapper")).toBe(true);
    expect(positioner.style.position).toBe("fixed");
    expect(dropdown.className).toContain("--radix-popover-trigger-width");
    expect(dropdown.className).toContain("--radix-popover-content-available-height");
    expect(dropdown.dataset.settingsSearchPortal).toBe("true");
    expect(positioner.parentElement).toBe(document.body);
  });

  it("keeps focus in the search field while the popover opens", async () => {
    const { user } = withUser(<SettingsView />);
    const search = screen.getByRole("searchbox", { name: "Search settings" });

    await user.type(search, "privacy");

    expect(document.activeElement).toBe(search);
    expect(screen.getByRole("region", { name: "Settings search results" })).toBeTruthy();
  });

  it("moves between the search field and results without clearing the popover", async () => {
    const { user } = withUser(<SettingsView />);
    const search = screen.getByRole<HTMLInputElement>("searchbox", { name: "Search settings" });
    await user.type(search, "storage quota");

    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(screen.getByRole("button", { name: /Storage quota/i }));

    await user.keyboard("{Escape}");
    expect(document.activeElement).toBe(search);
    expect(search.value).toBe("storage quota");
    expect(screen.getByRole("region", { name: "Settings search results" })).toBeTruthy();
  });

  it("keeps the popover open for anchor and focus interactions", async () => {
    const { user } = withUser(<SettingsView />);
    const search = screen.getByRole<HTMLInputElement>("searchbox", { name: "Search settings" });
    await user.type(search, "privacy");

    await user.click(search);
    screen.getByRole("tab", { name: "Appearance" }).focus();

    expect(search.value).toBe("privacy");
    expect(screen.getByRole("region", { name: "Settings search results" })).toBeTruthy();
  });

  it("dismisses the search popover on Escape or an outside pointer", async () => {
    const { user } = withUser(<SettingsView />);
    const search = screen.getByRole<HTMLInputElement>("searchbox", { name: "Search settings" });
    await user.type(search, "privacy");
    await user.keyboard("{Escape}");

    expect(search.value).toBe("");
    expect(document.activeElement).toBe(search);

    await user.type(search, "privacy");
    await user.click(screen.getByRole("tab", { name: "Appearance" }));

    await waitFor(() => expect(search.value).toBe(""));
    expect(screen.queryByRole("region", { name: "Settings search results" })).toBeNull();
  });

  it("clears a search and restores the settings navigation", async () => {
    const { user } = withUser(<SettingsView />);
    await user.type(screen.getByRole("searchbox", { name: "Search settings" }), "privacy");
    await user.click(screen.getByRole("button", { name: "Clear search" }));

    expect(screen.getByRole("tablist", { name: "Settings sections" })).toBeTruthy();
  });

  it("opens the result's tab and its matching setting", async () => {
    const { user } = withUser(<SettingsView />);
    await user.type(screen.getByRole("searchbox", { name: "Search settings" }), "screenshots");
    await user.click(await screen.findByRole("button", { name: /Allow screenshots/i }));

    expect(await screen.findByRole("switch", { name: "Allow screenshots" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "List" }).getAttribute("aria-selected")).toBe("true");
    await waitFor(() =>
      expect(
        screen.getByRole("switch", { name: "Allow screenshots" })
          .closest<HTMLElement>("[data-settings-search-target]")
          ?.dataset.settingsSearchHighlight,
      ).toBe("true"),
    );
    const row = screen.getByRole("switch", { name: "Allow screenshots" })
      .closest<HTMLElement>("[data-settings-search-target]")!;
    expect(row.className).toContain("px-s-3");
    expect(row.className).toContain("after:left-s-3");
  });

  it("opens Diagnostics when a recovery screen requests it", async () => {
    useUi.getState().setSettingsTab("diagnostics");
    withClient(<SettingsView />);

    await waitFor(() =>
      expect(screen.getByRole("tab", { name: "Diagnostics" }).getAttribute("aria-selected")).toBe("true"),
    );
    expect(useUi.getState().settingsTab).toBeNull();
  });

  it("keeps search useful on Android, including the persistent Service controls", async () => {
    setUserAgent("Mozilla/5.0 (Linux; Android 15)");
    const { user } = withUser(<SettingsView />);
    const search = screen.getByRole("searchbox", { name: "Search settings" });
    await user.type(search, "Capture from other apps");
    expect(await screen.findByText("Capture from other apps")).toBeTruthy();

    await user.clear(search);
    await user.type(search, "storage quota");
    expect(await screen.findByText("Storage quota")).toBeTruthy();

    await user.clear(search);
    await user.type(search, "translucency");
    expect(await screen.findByText("No settings found")).toBeTruthy();
  });
});

describe("appearance", () => {
  it("says which appearance System is using (CopyPaste-8ebg.63)", () => {
    withClient(<SettingsView />);
    expect(screen.getByText(/Using (Dark|Light) now\./)).toBeTruthy();
  });

  it("exposes each accent as a named, pressable control (A11Y-8, A11Y-9)", () => {
    withClient(<SettingsView />);
    const group = screen.getByRole("group", { name: "Accent" });
    const swatches = [...group.querySelectorAll<HTMLButtonElement>("button")];
    expect(swatches.map((swatch) => swatch.dataset.swatch)).toEqual(ACCENTS);
    for (const swatch of swatches) {
      expect(swatch.getAttribute("aria-label")?.length ?? 0).toBeGreaterThan(0);
      expect(swatch.getAttribute("aria-pressed")).toMatch(/true|false/);
    }
    const system = screen.getByRole("button", { name: "System accent" });
    expect(system.textContent).toContain("System");
    expect(group.querySelectorAll('[aria-pressed="true"]')).toHaveLength(1);
  });

  it("persists a change through the store", async () => {
    const { user } = withUser(<SettingsView />);
    await user.click(screen.getByRole("button", { name: "Teal" }));
    expect(usePrefs.getState().accent).toBe("teal");
  });

  it("lets macOS set the translucency intensity", async () => {
    const { user } = withUser(<SettingsView />);
    const slider = screen.getByRole("slider", { name: "Translucency" });
    await user.click(slider);
    await user.keyboard("{ArrowRight}".repeat(3));
    expect(usePrefs.getState().translucency).toBeGreaterThan(0);
    expect(screen.getByText(`${usePrefs.getState().translucency}%`)).toBeTruthy();
  });

  it("keeps every swatch colour immutable when another accent is selected", async () => {
    const { user } = withUser(<SettingsView />);
    const group = screen.getByRole("group", { name: "Accent" });
    const before = [...group.querySelectorAll<HTMLButtonElement>("button")].map(
      (swatch) => swatch.style.backgroundColor,
    );
    const swatches = [...group.querySelectorAll<HTMLButtonElement>("button")];

    expect(new Set(before)).toHaveLength(ACCENTS.length);
    for (const swatch of swatches) {
      expect(swatch.className).toContain("rounded-full");
      expect(swatch.className).toContain("focus-visible:ring-ring");
    }
    await user.click(screen.getByRole("button", { name: "Teal" }));
    expect([...group.querySelectorAll<HTMLButtonElement>("button")].map(
      (swatch) => swatch.style.backgroundColor,
    )).toEqual(before);
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
