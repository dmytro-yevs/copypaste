import { describe, expect, test } from "vitest";

import {
  settingsViewLevel,
  type SettingsViewSnapshot,
} from "../src/harness/settings.js";

describe("adaptive Preferences navigation", () => {
  const fixtures: ReadonlyArray<{
    name: string;
    snapshot: SettingsViewSnapshot;
    expected: ReturnType<typeof settingsViewLevel>;
  }> = [
    {
      name: "compact category menu",
      snapshot: { navigation: true, back: false, visiblePanels: [] },
      expected: "navigation",
    },
    {
      name: "compact detail",
      snapshot: {
        navigation: false,
        back: true,
        visiblePanels: ["Storage & history"],
      },
      expected: "detail",
    },
    {
      name: "expanded tabs and panel",
      snapshot: {
        navigation: true,
        back: false,
        visiblePanels: ["Appearance"],
      },
      expected: "navigation",
    },
    {
      name: "remount gap",
      snapshot: { navigation: false, back: false, visiblePanels: [] },
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
      settingsViewLevel({ navigation: false, back: true, visiblePanels: [] }),
    ).toBe("neither");
  });
});
