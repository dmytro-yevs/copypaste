import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { afterEach, describe, expect, it, vi } from "vitest";
import { minifySync } from "vite";

import {
  APPEARANCE_SERIALIZATION,
  translucencyAttribute,
  unwrapPersistedPrefs,
} from "@/lib/appearancePrefs";
import { resolveTheme } from "@/lib/theme";
import { themeBootstrapSource } from "@/lib/themeBootstrapSource";
import { parsePrefs } from "@/store/prefs";

const BOOTSTRAP_SOURCE = readFileSync(
  resolve(process.cwd(), "public/theme-bootstrap.js"),
  "utf8",
);
const INDEX_HTML = readFileSync(resolve(process.cwd(), "index.html"), "utf8");
const MAIN_SOURCE = readFileSync(resolve(process.cwd(), "src/main.tsx"), "utf8");
const TAURI_CONFIG = JSON.parse(
  readFileSync(resolve(process.cwd(), "src-tauri/tauri.conf.json"), "utf8"),
) as {
  app: {
    security: {
      csp: Record<string, string[]>;
      devCsp: Record<string, string[]>;
    };
  };
};

function setMatchMedia(matches: boolean): void {
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    value: vi.fn(() => ({ matches })),
  });
}

function runBootstrap(stored?: unknown): DOMStringMap {
  const root = document.documentElement;
  for (const key of [
    "theme",
    "themePref",
    "accent",
    "translucency",
    "themeBootstrapped",
  ]) {
    delete root.dataset[key];
  }
  localStorage.clear();
  if (stored !== undefined) {
    localStorage.setItem(
      APPEARANCE_SERIALIZATION.storageKey,
      typeof stored === "string" ? stored : JSON.stringify(stored),
    );
  }
  window.eval(BOOTSTRAP_SOURCE);
  return root.dataset;
}

afterEach(() => {
  vi.restoreAllMocks();
  localStorage.clear();
});

describe("appearance bootstrap artifact", () => {
  it("is generated from the maintained serialization contract (AT-54)", () => {
    const minify = (source: string) =>
      minifySync("theme-bootstrap.js", source, { module: false }).code;
    expect(minify(BOOTSTRAP_SOURCE)).toBe(minify(themeBootstrapSource()));
  });

  it("is a dependency-free classic script allowed by the configured CSP", () => {
    const code = BOOTSTRAP_SOURCE.replace(/\/\*[\s\S]*?\*\//g, "");
    expect(code).not.toMatch(/\bimport\b|\beval\s*\(|new\s+Function/);

    for (const csp of [TAURI_CONFIG.app.security.csp, TAURI_CONFIG.app.security.devCsp]) {
      const scriptPolicy = csp["script-src"] ?? csp["default-src"];
      expect(scriptPolicy).toContain("'self'");
      expect(scriptPolicy).not.toContain("'unsafe-inline'");
      expect(scriptPolicy).not.toContain("'unsafe-eval'");
    }
  });
});

describe("first-frame ordering (INV-22 / AT-49)", () => {
  it("loads and marks a persisted light appearance before the app module", () => {
    const bootstrapTag = '<script vite-ignore src="./theme-bootstrap.js"></script>';
    const moduleTag = '<script type="module" src="/src/main.tsx"></script>';
    expect(INDEX_HTML.indexOf(bootstrapTag)).toBeGreaterThan(-1);
    expect(INDEX_HTML.indexOf(bootstrapTag)).toBeLessThan(INDEX_HTML.indexOf(moduleTag));
    expect(MAIN_SOURCE.indexOf("themeBootstrapped")).toBeLessThan(
      MAIN_SOURCE.indexOf("createRoot(root).render"),
    );

    setMatchMedia(false);
    const dataset = runBootstrap({
      state: { theme: "light", accent: "amber", translucency: true },
      version: 0,
    });
    expect(dataset).toMatchObject({
      theme: "light",
      themePref: "light",
      accent: "amber",
      translucency: "on",
      themeBootstrapped: "1",
    });
  });
});

describe("bootstrap/runtime schema parity", () => {
  it("matches runtime parsing for envelopes, enums, invalid fields, and defaults", () => {
    setMatchMedia(false);
    const storedValues: unknown[] = [
      undefined,
      ...APPEARANCE_SERIALIZATION.themes.map((theme) => ({ state: { theme }, version: 0 })),
      ...APPEARANCE_SERIALIZATION.accents.map((accent) => ({ accent })),
      { translucency: true },
      { translucency: false },
      { state: { theme: "chartreuse", accent: "teal", translucency: 1 }, version: 0 },
    ];
    vi.spyOn(console, "warn").mockImplementation(() => {});

    for (const stored of storedValues) {
      const runtime = parsePrefs(
        unwrapPersistedPrefs(stored === undefined ? undefined : stored),
      );
      const dataset = runBootstrap(stored);
      expect(
        {
          theme: dataset.theme,
          themePref: dataset.themePref,
          accent: dataset.accent,
          translucency: dataset.translucency,
        },
        JSON.stringify(stored),
      ).toEqual({
        theme: resolveTheme(runtime.theme),
        themePref: runtime.theme,
        accent: runtime.accent,
        translucency: translucencyAttribute(runtime.translucency),
      });
    }
  });

  it("resolves system appearance synchronously and survives unavailable state", () => {
    setMatchMedia(false);
    expect(runBootstrap({ state: { theme: "system" }, version: 0 }).theme).toBe("light");
    setMatchMedia(true);
    expect(runBootstrap({ state: { theme: "system" }, version: 0 }).theme).toBe("dark");

    expect(() => runBootstrap("{not json")).not.toThrow();
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new DOMException("denied", "SecurityError");
    });
    expect(() => runBootstrap()).not.toThrow();
    expect(document.documentElement.dataset.themeBootstrapped).toBe("1");
  });
});
