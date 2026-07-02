import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  ACCENT_VALUES,
  DEFAULT_ACCENT,
  DEFAULT_THEME,
  DEFAULT_TRANSLUCENCY,
  PREFS_KEY,
  THEME_VALUES,
  translucencyAttr,
} from "./prefsSchema";

// The pre-paint bootstrap is a standalone classic asset that CANNOT import, so
// it re-declares the prefs KEY / defaults / allowed values / translucency
// mapping as literals. This suite runs the REAL public/theme-bootstrap.js in
// jsdom and cross-checks its behavior against prefsSchema (the single source of
// truth) — that is the anti-drift guarantee (design.md Decision 4 · task 1.14).

// Resolved from the vitest root (the copypaste-ui crate dir = process.cwd());
// jsdom rewrites import.meta.url to a non-file URL, so file-URL resolution fails.
const BOOTSTRAP_SRC = readFileSync(
  resolve(process.cwd(), "public/theme-bootstrap.js"),
  "utf8",
);

/** Reset <html> dataset + storage, seed a value, run the bootstrap fresh. */
function runBootstrap(stored?: unknown): DOMStringMap {
  const el = document.documentElement;
  delete el.dataset.theme;
  delete el.dataset.themePref;
  delete el.dataset.accent;
  delete el.dataset.translucency;
  delete el.dataset.themeBootstrapped;
  localStorage.clear();
  if (stored !== undefined) {
    localStorage.setItem(
      PREFS_KEY,
      typeof stored === "string" ? stored : JSON.stringify(stored),
    );
  }
  // Direct eval runs the IIFE against jsdom's globals (localStorage, document).
  (0, eval)(BOOTSTRAP_SRC);
  return el.dataset;
}

/** Minimal MediaQueryList mock — enough for the bootstrap's `.matches` read. */
function mockMatchMedia(prefersDark: boolean): void {
  window.matchMedia = ((): MediaQueryList =>
    ({
      matches: prefersDark,
      media: "(prefers-color-scheme: dark)",
      addEventListener: () => {},
      removeEventListener: () => {},
    }) as unknown as MediaQueryList) as unknown as typeof window.matchMedia;
}

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  const el = document.documentElement;
  delete el.dataset.theme;
  delete el.dataset.themePref;
  delete el.dataset.accent;
  delete el.dataset.translucency;
  delete el.dataset.themeBootstrapped;
  // @ts-expect-error — test-only cleanup of a jsdom global that isn't
  // implemented by default.
  delete window.matchMedia;
});

// Strip comments so the "no import/eval/Function" check inspects CODE only —
// the file's own doc comment legitimately mentions those words in prose.
const BOOTSTRAP_CODE = BOOTSTRAP_SRC.replace(/\/\*[\s\S]*?\*\//g, "").replace(
  /\/\/[^\n]*/g,
  "",
);

describe("theme-bootstrap.js — static constraints", () => {
  it("contains no import / eval / new Function (must stay a classic asset)", () => {
    expect(BOOTSTRAP_CODE).not.toMatch(/\bimport\b/);
    expect(BOOTSTRAP_CODE).not.toMatch(/\beval\s*\(/);
    expect(BOOTSTRAP_CODE).not.toMatch(/new\s+Function/);
  });
});

describe("theme-bootstrap.js — anti-drift parity with prefsSchema", () => {
  it("uses the same storage KEY (seeding under PREFS_KEY is read back)", () => {
    const ds = runBootstrap({ theme: "light", accent: "teal", translucency: false });
    expect(ds.theme).toBe("light");
    expect(ds.themePref).toBe("light");
    expect(ds.accent).toBe("teal");
    expect(ds.translucency).toBe("off");
  });

  it("defaults match prefsSchema defaults + translucency mapping (empty storage)", () => {
    const ds = runBootstrap();
    expect(ds.theme).toBe(DEFAULT_THEME);
    expect(ds.themePref).toBe(DEFAULT_THEME);
    expect(ds.accent).toBe(DEFAULT_ACCENT);
    expect(ds.translucency).toBe(translucencyAttr(DEFAULT_TRANSLUCENCY));
  });

  it("accepts exactly the schema's allowed theme values (\"system\" resolves; data-theme-pref keeps the raw choice)", () => {
    for (const theme of THEME_VALUES) {
      if (theme === "system") continue; // covered explicitly below (needs matchMedia control)
      const ds = runBootstrap({ theme });
      expect(ds.theme).toBe(theme);
      expect(ds.themePref).toBe(theme);
    }
  });

  it('"system" resolves data-theme via matchMedia("(prefers-color-scheme: dark)"), keeping data-theme-pref="system"', () => {
    mockMatchMedia(true);
    let ds = runBootstrap({ theme: "system" });
    expect(ds.theme).toBe("dark");
    expect(ds.themePref).toBe("system");

    mockMatchMedia(false);
    ds = runBootstrap({ theme: "system" });
    expect(ds.theme).toBe("light");
    expect(ds.themePref).toBe("system");
  });

  it('"system" resolves data-theme to "dark" when matchMedia is unavailable (matches prefsSchema resolveTheme guard)', () => {
    const ds = runBootstrap({ theme: "system" });
    expect(ds.theme).toBe("dark");
    expect(ds.themePref).toBe("system");
  });

  it("accepts exactly the schema's allowed accent values", () => {
    for (const accent of ACCENT_VALUES) {
      expect(runBootstrap({ accent }).accent).toBe(accent);
    }
  });

  it("maps translucency boolean → on/off exactly like translucencyAttr", () => {
    expect(runBootstrap({ translucency: true }).translucency).toBe(translucencyAttr(true));
    expect(runBootstrap({ translucency: false }).translucency).toBe(translucencyAttr(false));
  });
});

describe("theme-bootstrap.js — defensive behavior", () => {
  it("always sets the themeBootstrapped ordering marker", () => {
    expect(runBootstrap().themeBootstrapped).toBe("1");
    expect(runBootstrap({ theme: "light" }).themeBootstrapped).toBe("1");
  });

  it("malformed JSON → defaults, marker still set, no throw", () => {
    const ds = runBootstrap("{not json");
    expect(ds.theme).toBe(DEFAULT_THEME);
    expect(ds.themePref).toBe(DEFAULT_THEME);
    expect(ds.accent).toBe(DEFAULT_ACCENT);
    expect(ds.translucency).toBe("on");
    expect(ds.themeBootstrapped).toBe("1");
  });

  it("each field invalid defaults independently, valid siblings kept", () => {
    // "sepia" is not in THEMES (unlike "system", which is now a valid choice).
    const ds = runBootstrap({ theme: "sepia", accent: "teal", translucency: 1 });
    expect(ds.theme).toBe(DEFAULT_THEME); // invalid → default
    expect(ds.themePref).toBe(DEFAULT_THEME); // invalid → default
    expect(ds.accent).toBe("teal"); // valid → kept
    expect(ds.translucency).toBe("on"); // non-boolean → default true → "on"
  });

  it("missing storage → defaults", () => {
    const ds = runBootstrap();
    expect(ds.theme).toBe(DEFAULT_THEME);
    expect(ds.accent).toBe(DEFAULT_ACCENT);
  });

  it("a localStorage access exception → defaults, marker set, no throw", () => {
    const orig = localStorage.getItem;
    localStorage.getItem = () => {
      throw new Error("SecurityError");
    };
    try {
      const el = document.documentElement;
      delete el.dataset.theme;
      delete el.dataset.themeBootstrapped;
      (0, eval)(BOOTSTRAP_SRC);
      expect(el.dataset.theme).toBe(DEFAULT_THEME);
      expect(el.dataset.accent).toBe(DEFAULT_ACCENT);
      expect(el.dataset.themeBootstrapped).toBe("1");
    } finally {
      localStorage.getItem = orig;
    }
  });
});

describe("index.html / popup.html static defaults — anti-drift with prefsSchema", () => {
  // The HTML files carry a static data-theme/accent/translucency as the pre-bootstrap
  // fallback. That's a third copy of the defaults (beyond prefsSchema + the bootstrap);
  // assert it matches so it can't silently drift if DEFAULT_* changes.
  for (const file of ["index.html", "popup.html"]) {
    it(`${file} static <html> defaults match schema defaults`, () => {
      const html = readFileSync(resolve(process.cwd(), file), "utf8");
      const htmlTag = html.slice(html.indexOf("<html"), html.indexOf(">", html.indexOf("<html")) + 1);
      expect(htmlTag).toContain(`data-theme="${DEFAULT_THEME}"`);
      expect(htmlTag).toContain(`data-accent="${DEFAULT_ACCENT}"`);
      expect(htmlTag).toContain(`data-translucency="${translucencyAttr(DEFAULT_TRANSLUCENCY)}"`);
    });
  }
});
