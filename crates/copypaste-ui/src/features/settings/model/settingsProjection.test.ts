import { describe, expect, it } from "vitest";
import { settingsGroups } from "./settingsProjection";
import { settingDefinition, SETTINGS_FIELD_DEFINITIONS, SETTINGS_SECTIONS } from "./settingsSchemaCatalog";
import { SETTINGS_SEARCH_ITEMS } from "./settingsSearchIndex";

const identity = (key: string) => key;

describe("settings schema projection", () => {
  it("keeps nine sections and unique stable field identities", () => {
    expect(SETTINGS_SECTIONS.map((section) => section.value)).toEqual([
      "appearance", "clipboard", "privacy", "shortcuts", "device-sync",
      "cloud-sync", "storage", "diagnostics", "about",
    ]);
    expect(new Set(SETTINGS_FIELD_DEFINITIONS.map((field) => field.id)).size).toBe(SETTINGS_FIELD_DEFINITIONS.length);
  });

  it("projects fields in canonical order regardless of binding order", () => {
    const groups = settingsGroups("clipboard", [
      { kind: "boolean", definition: settingDefinition("clipboard", "settings.service.sound.title"), value: false, onChange: () => {} },
      { kind: "boolean", definition: settingDefinition("clipboard", "settings.service.notify.title"), value: true, onChange: () => {} },
    ], identity);
    expect(groups).toHaveLength(1);
    expect(groups[0]?.id).toBe("notifications");
    expect(groups[0]?.fields.map((field) => field.definition.title)).toEqual([
      "settings.service.notify.title", "settings.service.sound.title",
    ]);
  });

  it("gives hidden search rows a stable canonical group destination", () => {
    const setup = SETTINGS_SEARCH_ITEMS.find((item) => item.title === "settings.sync.cloud.setupTitle");
    const syncNow = SETTINGS_SEARCH_ITEMS.find((item) => item.title === "settings.sync.now.title");
    expect(setup?.section).toBe("settings.sync.cloud.sectionTitle");
    expect(syncNow?.section).toBe("Devices");
    expect(SETTINGS_SEARCH_ITEMS.some((item) => item.title === "capture.setup.enable.title")).toBe(false);
    expect(SETTINGS_SEARCH_ITEMS.filter((item) => item.title === "settings.service.exclusions.title")).toHaveLength(1);
  });

  it("rejects a control kind that violates the catalog", () => {
    expect(() => settingsGroups("clipboard", [
      { kind: "readonly", definition: settingDefinition("clipboard", "settings.service.sound.title"), value: "off" },
    ], identity)).toThrow(/expects boolean/);
  });
});
