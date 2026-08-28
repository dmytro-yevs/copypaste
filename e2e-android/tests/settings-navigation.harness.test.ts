import { describe, expect, test } from "vitest";

import type { Item } from "../../crates/copypaste-ui/src/generated/ipc.js";
import { missingFixtureIds } from "../src/harness/bridge.js";
import {
  settingsNavigationAction,
  settingsNavigationReady,
  settingsPanelReady,
  settingsScrollDelta,
  settingsSliderIndex,
  settingsTriggerDecision,
  settingsViewLevel,
  type SettingsTriggerSnapshot,
  type SettingsViewSnapshot,
} from "../src/harness/settings.js";

function item(id: string, content: string | null, contentType = "text"): Item {
  return {
    id,
    content,
    content_type: contentType,
    created_at: 0,
    pinned: false,
    is_sensitive: content === null,
    sensitive_finding: null,
    origin_device_id: "device",
    origin_device_name: null,
    source_app_bundle_id: null,
    source_app_name: null,
    too_large_to_sync: false,
    truncated: false,
  };
}

describe("the Settings history fixture query", () => {
  test("retains all 101 seeded ids alongside null, binary, and unknown items", () => {
    const fixtureIds = Array.from({ length: 101 }, (_, index) => `fixture-${index}`);
    const stored: Item[] = [
      item("sensitive", null),
      item("image", null, "image/png"),
      item("other", "opaque", "application/octet-stream"),
      ...fixtureIds.map((id) => item(id, `${id} text`)),
    ];

    expect(missingFixtureIds(stored, fixtureIds)).toEqual([]);
    expect(stored).toHaveLength(104);
  });
});

describe("adaptive Preferences navigation", () => {
  const fixtures: ReadonlyArray<{
    name: string;
    snapshot: SettingsViewSnapshot;
    expected: ReturnType<typeof settingsViewLevel>;
  }> = [
    {
      name: "compact category menu",
      snapshot: {
        navigation: true,
        back: false,
        visiblePanels: [],
        busyPanels: [],
        scrollTop: 0,
      },
      expected: "navigation",
    },
    {
      name: "compact detail",
      snapshot: {
        navigation: false,
        back: true,
        visiblePanels: ["Storage & history"],
        busyPanels: [],
        scrollTop: 640,
      },
      expected: "detail",
    },
    {
      name: "expanded tabs and panel",
      snapshot: {
        navigation: true,
        back: false,
        visiblePanels: ["Appearance"],
        busyPanels: [],
        scrollTop: 0,
      },
      expected: "navigation",
    },
    {
      name: "remount gap",
      snapshot: {
        navigation: false,
        back: false,
        visiblePanels: [],
        busyPanels: [],
        scrollTop: null,
      },
      expected: "neither",
    },
  ];

  for (const fixture of fixtures) {
    test(`recognises the ${fixture.name}`, () => {
      expect(settingsViewLevel(fixture.snapshot)).toBe(fixture.expected);
    });
  }

  test("does not mistake a detached Back control for an open detail", () => {
    expect(
      settingsViewLevel({
        navigation: false,
        back: true,
        visiblePanels: [],
        busyPanels: [],
        scrollTop: 320,
      }),
    ).toBe("neither");
  });

  test("requires compact Back to restore the actual viewport to the top", () => {
    const detail: SettingsViewSnapshot = {
      navigation: false,
      back: true,
      visiblePanels: ["Storage & history"],
      busyPanels: [],
      scrollTop: 640,
    };
    expect(settingsNavigationAction(detail, false)).toBe("restore");
    expect(settingsNavigationAction({ ...detail, scrollTop: 0 }, false)).toBe("back");

    const restoredNavigation: SettingsViewSnapshot = {
      navigation: true,
      back: false,
      visiblePanels: [],
      busyPanels: [],
      scrollTop: 0,
    };
    expect(settingsNavigationAction({ ...restoredNavigation, scrollTop: 640 }, true)).toBe("wait");
    expect(settingsNavigationAction(restoredNavigation, true)).toBe("ready");
    expect(settingsNavigationReady({ ...restoredNavigation, scrollTop: 480 }, true)).toBe(false);
    expect(settingsNavigationReady({ ...restoredNavigation, scrollTop: null }, true)).toBe(false);
  });

  test("waits for an opened panel to finish its asynchronous content", () => {
    const opened: SettingsViewSnapshot = {
      navigation: false,
      back: true,
      visiblePanels: ["Diagnostics"],
      busyPanels: ["Diagnostics"],
      scrollTop: 0,
    };
    expect(settingsPanelReady(opened, "Diagnostics")).toBe(false);
    expect(settingsPanelReady({ ...opened, busyPanels: [] }, "Diagnostics")).toBe(true);
    expect(settingsPanelReady({ ...opened, busyPanels: [] }, "About")).toBe(false);
  });
});

describe("a Preferences category trigger", () => {
  const actionable: SettingsTriggerSnapshot = {
    exists: true,
    disabled: false,
    ariaDisabled: null,
    width: 280,
    height: 64,
    top: 120,
    viewportTop: 80,
    viewportHeight: 640,
    clipped: 0,
    documentOverflow: 0,
    centerInsideViewport: true,
    centerHit: true,
  };

  test("taps an expanded tab whose centre owns the hit", () => {
    expect(settingsTriggerDecision(actionable)).toBe("tap");
  });

  test("scrolls a lower compact category into the viewport before tapping", () => {
    const lower = {
      ...actionable,
      top: 900,
      viewportTop: 100,
      viewportHeight: 600,
      centerInsideViewport: false,
      centerHit: false,
    };
    expect(settingsTriggerDecision(lower)).toBe("scroll");
    expect(settingsScrollDelta(lower)).toBe(532);
  });

  test("does not tap through an element covering the category centre", () => {
    expect(settingsTriggerDecision({ ...actionable, centerHit: false })).toBe("scroll");
  });

  test("does not treat absent or disabled controls as scroll work", () => {
    expect(settingsTriggerDecision({ ...actionable, exists: false })).toBe("missing");
    expect(settingsTriggerDecision({ ...actionable, disabled: true })).toBe("blocked");
    expect(settingsTriggerDecision({ ...actionable, ariaDisabled: "true" })).toBe("blocked");
  });
});

describe("the History display slider", () => {
  const valid = {
    exists: true,
    index: "2",
    min: "0",
    max: "4",
    output: "500",
  };

  test("accepts an integer index inside its declared range", () => {
    expect(settingsSliderIndex(valid)).toBe(2);
  });

  test.each([
    ["missing slider", { ...valid, exists: false }],
    ["missing index", { ...valid, index: null }],
    ["empty index", { ...valid, index: "" }],
    ["missing minimum", { ...valid, min: null }],
    ["missing maximum", { ...valid, max: null }],
  ])("rejects a %s with its rendered output in diagnostics", (_name, snapshot) => {
    expect(() => settingsSliderIndex(snapshot)).toThrow(
      '"output":"500"',
    );
  });
});
