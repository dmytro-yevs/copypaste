/**
 * The compact settings ladder, driven through the view that owns its state.
 * DMY-154: the nine-item strip was width-independent, so every label got a
 * sliver of one row on a phone. The ladder replaces it below the boundary —
 * and, being width-driven, keeps the two-pane layout on a tablet.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor } from "@testing-library/react";

import { SettingsView } from "@/components/settings/SettingsView";
import { DEFAULT_PREFS, usePrefs } from "@/store/prefs";
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
describe("the compact settings ladder", () => {
  const PHONE = 360;

  it("replaces the tab strip with an index of grouped sections", () => {
    setViewportWidth(PHONE);
    withClient(<SettingsView />);

    expect(screen.queryByRole("tablist")).toBeNull();
    expect(screen.queryAllByRole("tab")).toHaveLength(0);
    expect(screen.getByRole("navigation", { name: "Settings sections" })).toBeTruthy();
    for (const group of ["Personal", "CopyPaste", "Support"]) {
      expect(screen.getByRole("heading", { name: group })).toBeTruthy();
    }
    for (const section of ["Appearance", "List", "Service", "Storage", "About"]) {
      expect(screen.getByRole("button", { name: section })).toBeTruthy();
    }
  });

  it("takes a compact desktop window down the same ladder", () => {
    setUserAgent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)");
    setViewportWidth(PHONE);
    withClient(<SettingsView />);

    expect(screen.queryByRole("tablist")).toBeNull();
    expect(screen.getByRole("navigation", { name: "Settings sections" })).toBeTruthy();
  });

  it("opens a section in one activation", async () => {
    setViewportWidth(PHONE);
    const { user } = withUser(<SettingsView />);

    await user.click(screen.getByRole("button", { name: "Storage" }));

    expect(screen.getByRole("heading", { name: "Storage", level: 2 })).toBeTruthy();
    expect(await screen.findByRole("button", { name: "Export…" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Appearance" })).toBeNull();
  });

  it("restores the level and the focus when Back is activated", async () => {
    setViewportWidth(PHONE);
    const { user } = withUser(<SettingsView />);
    await user.click(screen.getByRole("button", { name: "Storage" }));

    await user.click(screen.getByRole("button", { name: "All settings" }));

    await waitFor(() =>
      expect(screen.getByRole("navigation", { name: "Settings sections" })).toBeTruthy(),
    );
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Storage" }));
  });

  /**
   * `WryActivity.kt` routes the system Back gesture to `WebView.goBack()`, so a
   * subpage that is not a history entry closes the app instead of going up one
   * level.
   */
  it("pushes a history entry the system Back gesture can pop", async () => {
    setViewportWidth(PHONE);
    const pushState = vi.spyOn(window.history, "pushState");
    const { user } = withUser(<SettingsView />);

    await user.click(screen.getByRole("button", { name: "Storage" }));
    expect(pushState).toHaveBeenCalledTimes(1);

    act(() => window.dispatchEvent(new PopStateEvent("popstate")));

    expect(screen.getByRole("navigation", { name: "Settings sections" })).toBeTruthy();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Storage" }));
  });

  it("reaches every section in one activation from the index", async () => {
    setUserAgent("Mozilla/5.0 (Linux; Android 15)");
    setViewportWidth(PHONE);
    const { user } = withUser(<SettingsView />);
    const sections = [
      "Appearance",
      "List",
      "Service",
      "Background capture",
      "Sync",
      "Storage",
      "Diagnostics",
      "About",
    ];

    for (const section of sections) {
      await user.click(screen.getByRole("button", { name: section }));
      expect(screen.getByRole("heading", { name: section, level: 2 })).toBeTruthy();
      await user.click(screen.getByRole("button", { name: "All settings" }));
      await waitFor(() =>
        expect(screen.getByRole("navigation", { name: "Settings sections" })).toBeTruthy(),
      );
    }
  });

  it("opens the requested section directly when a recovery screen asks", async () => {
    setViewportWidth(PHONE);
    useUi.getState().setSettingsTab("diagnostics");
    withClient(<SettingsView />);

    expect(
      await screen.findByRole("heading", { name: "Diagnostics", level: 2 }),
    ).toBeTruthy();
    expect(useUi.getState().settingsTab).toBeNull();
  });

  it("opens the section a search result belongs to", async () => {
    setViewportWidth(PHONE);
    const { user } = withUser(<SettingsView />);
    await user.type(screen.getByRole("searchbox", { name: "Search settings" }), "screenshots");

    await user.click(await screen.findByRole("button", { name: /Allow screenshots/i }));

    expect(await screen.findByRole("switch", { name: "Allow screenshots" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "List", level: 2 })).toBeTruthy();
  });

  /**
   * The entry pushed for a subpage has to leave with it. Rotating out of the
   * ladder fires no `popstate`, so an entry left behind is spent by the next
   * system Back — which the user sees as Back doing nothing.
   */
  it("keeps Back working across a rotation out of a subpage and back", async () => {
    setViewportWidth(PHONE);
    const pushState = vi.spyOn(window.history, "pushState");
    const back = vi.spyOn(window.history, "back").mockImplementation(() => {});
    const { user } = withUser(<SettingsView />);
    await user.click(screen.getByRole("button", { name: "Storage" }));
    expect(pushState).toHaveBeenCalledTimes(1);

    setViewportWidth(891);
    expect(back).toHaveBeenCalledTimes(1);

    setViewportWidth(PHONE);
    expect(pushState).toHaveBeenCalledTimes(2);

    act(() => window.dispatchEvent(new PopStateEvent("popstate")));

    expect(screen.getByRole("navigation", { name: "Settings sections" })).toBeTruthy();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Storage" }));
  });

  it("carries the open section into the two-pane layout on a rotation", async () => {
    setViewportWidth(PHONE);
    const { user } = withUser(<SettingsView />);
    await user.click(screen.getByRole("button", { name: "Storage" }));

    setViewportWidth(891);

    expect(screen.getByRole("tab", { name: "Storage" }).getAttribute("aria-selected")).toBe(
      "true",
    );
  });
});

