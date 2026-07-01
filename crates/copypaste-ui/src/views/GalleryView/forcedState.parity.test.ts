import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

// ---------------------------------------------------------------------------
// Forced-state CSS parity (task 6.8, design.md Decision 7/G1).
//
// The gallery renders a representative set of components with
// `data-force-state="hover"|"active"|"focus"` (ForcedStateSection.tsx / gallery
// matrix cells) so a static page can still show what the real `:hover`/
// `:active`/`:focus-visible` pseudo-class looks like. This test asserts each
// `[data-force-state=…]` rule in gallery.css declares EXACTLY the same
// property values as the real pseudo-class rule it mirrors, in
// primitives.css/patterns.css/base.css — so the two can never silently drift
// apart (a later Playwright interaction-screenshot test, task 6.13, is the
// complementary end-to-end check).
// ---------------------------------------------------------------------------

const GALLERY_CSS = readFileSync(resolve(process.cwd(), "src/styles/gallery.css"), "utf8");
const PRIMITIVES_CSS = readFileSync(resolve(process.cwd(), "src/styles/primitives.css"), "utf8");
const PATTERNS_CSS = readFileSync(resolve(process.cwd(), "src/styles/patterns.css"), "utf8");
const BASE_CSS = readFileSync(resolve(process.cwd(), "src/styles/base.css"), "utf8");

const canon = (s: string): string => s.replace(/\s+/g, "").toLowerCase();

function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/** Extract the declaration body of the FIRST `{selector} { … }` occurrence. */
function extractDecl(css: string, selector: string): string {
  const re = new RegExp(`${escapeRegExp(selector)}\\s*\\{([^}]*)\\}`);
  const m = css.match(re);
  if (!m) throw new Error(`selector not found in source CSS: "${selector}"`);
  return canon(m[1]);
}

// [real pseudo-class selector, its source CSS, the gallery.css forced twin]
const PAIRS: Array<[string, string, string]> = [
  [".btn--primary:hover", PRIMITIVES_CSS, '.gallery .btn--primary[data-force-state="hover"]'],
  [".btn--secondary:hover", PRIMITIVES_CSS, '.gallery .btn--secondary[data-force-state="hover"]'],
  [".btn:active", PRIMITIVES_CSS, '.gallery .btn[data-force-state="active"]'],
  [".iconbtn:hover", PRIMITIVES_CSS, '.gallery .iconbtn[data-force-state="hover"]'],
  [".iconbtn:active", PRIMITIVES_CSS, '.gallery .iconbtn[data-force-state="active"]'],
  [".row:hover", PATTERNS_CSS, '.gallery .row[data-force-state="hover"]'],
  [".chip:hover", PRIMITIVES_CSS, '.gallery .chip[data-force-state="hover"]'],
  [":focus-visible", BASE_CSS, '.gallery [data-force-state="focus"]'],
];

describe("gallery.css forced-state parity (task 6.8)", () => {
  it.each(PAIRS)(
    "%s declarations match its data-force-state twin",
    (realSelector, realCss, forcedSelector) => {
      const real = extractDecl(realCss, realSelector);
      const forced = extractDecl(GALLERY_CSS, forcedSelector);
      expect(forced).toBe(real);
    },
  );

  it("guards against a no-op extraction (fails loudly if parsing breaks)", () => {
    const sample = ".x:hover { color: red; }";
    expect(extractDecl(sample, ".x:hover")).toBe("color:red;");
  });
});
