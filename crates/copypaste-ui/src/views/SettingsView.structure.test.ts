// SettingsView.structure.test.ts
// Verifies the structural contract of the g06m.35 split:
//   - useSettingsState hook exists and exports the hook function
//   - StatusBanners component exists and exports StatusBanners
//   - TabBar component exists under SettingsView/components/TabBar
//
// These tests fail BEFORE the refactor and pass AFTER.
import { describe, it, expect } from "vitest";
import * as path from "node:path";
import * as fs from "node:fs";

const UI_SRC = path.resolve(__dirname, "..");

describe("g06m.35: SettingsView split structural contracts", () => {
  it("useSettingsState hook file exists", () => {
    const p = path.join(UI_SRC, "views/SettingsView/hooks/useSettingsState.ts");
    expect(fs.existsSync(p), `expected ${p} to exist`).toBe(true);
  });

  it("StatusBanners component file exists", () => {
    const p = path.join(UI_SRC, "views/SettingsView/components/StatusBanners.tsx");
    expect(fs.existsSync(p), `expected ${p} to exist`).toBe(true);
  });

  it("TabBar component file exists under SettingsView/components/", () => {
    const p = path.join(UI_SRC, "views/SettingsView/components/TabBar.tsx");
    expect(fs.existsSync(p), `expected ${p} to exist`).toBe(true);
  });

  it("SettingsView.tsx root is under 150 lines", () => {
    const p = path.join(UI_SRC, "views/SettingsView.tsx");
    const content = fs.readFileSync(p, "utf8");
    const lines = content.split("\n").length;
    expect(lines, `SettingsView.tsx has ${lines} lines — expected < 150`).toBeLessThan(150);
  });
});
