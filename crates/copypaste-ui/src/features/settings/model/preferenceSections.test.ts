import { describe, expect, it } from "vitest";

import { SETTINGS_SEARCH_ITEMS } from "./settingsSearchIndex";
import {
  preferenceSectionForTab,
  visiblePreferenceSections,
} from "./preferenceSections";
import { settingsCapabilities } from "./settingsNavigation";

describe("visiblePreferenceSections", () => {
  it("does not offer desktop shortcuts on Android", () => {
    const sections = visiblePreferenceSections(settingsCapabilities("android"));

    expect(sections.map((section) => section.value)).not.toContain("shortcuts");
  });

  it("keeps shortcuts in the desktop preference navigation", () => {
    const sections = visiblePreferenceSections(settingsCapabilities("macos"));

    expect(sections.map((section) => section.value)).toContain("shortcuts");
  });

  it("offers in-app updates on every shipped platform", () => {
    for (const platform of ["macos", "windows", "android"] as const) {
      expect(settingsCapabilities(platform).updater).toBe(true);
    }
    expect(settingsCapabilities("browser").updater).toBe(false);
  });

  it("keeps transfer actions inside Storage & history", () => {
    const sections = visiblePreferenceSections(settingsCapabilities("macos"));
    const transferItems = SETTINGS_SEARCH_ITEMS.filter((item) =>
      item.title.startsWith("settings.transfer."),
    );

    expect(sections.map((section) => section.value)).not.toContain("transfer");
    expect(sections.find((section) => section.value === "storage")?.label)
      .toBe("Storage & history");
    expect(transferItems).toHaveLength(4);
    expect(new Set(transferItems.map((item) => item.tab))).toEqual(
      new Set(["storage"]),
    );
    expect(SETTINGS_SEARCH_ITEMS.map((item) => String(item.tab)))
      .not.toContain("transfer");
  });

  it("maps old transfer destinations to Storage & history", () => {
    expect(preferenceSectionForTab("data-transfer")).toBe("storage");
    expect(preferenceSectionForTab("transfer")).toBe("storage");
  });
});
