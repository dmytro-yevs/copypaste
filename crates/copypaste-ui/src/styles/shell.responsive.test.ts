import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

// Regression coverage for CopyPaste-g27b.31 — Settings chrome breaks at the
// app's real minimum width (720px per tauri.conf.json) and at narrow widths:
//   (a) `.set-tabs` (7 Settings sub-tabs) overflowed with a hidden
//       (scrollbar-width:none) horizontal scroll — "Logs" was fully
//       off-screen and "About" was truncated. Fix: let the tab row wrap.
//   (b) `.about__links` (github / Changelog / Privacy policy) did not wrap
//       and spilled past the `.about` pane at narrow width.
//   (c) `.logs` log body: the message `code` element scrolled fully out of
//       view and the Refresh/Export toolbar overflowed instead of
//       reflowing; the `.lvl` level badge had no fixed width, so message
//       text left-aligned raggedly.
//
// jsdom does not implement real layout (scrollWidth/clientWidth are always
// 0), so — matching the existing text-based CSS convention in
// tokens.parity.test.ts — this asserts directly on the declared CSS rules in
// shell.css rather than on rendered DOM geometry.

const SHELL_CSS = readFileSync(
  resolve(process.cwd(), "src/styles/shell.css"),
  "utf8",
);

/**
 * Extract the declaration body of the first `selector { ... }` rule whose
 * prelude exactly matches `selector` (post) trimmed. shell.css has no
 * nested @media/@supports rules, so a single brace-matching pass suffices
 * (mirrors the simpler half of tokens.parity.test.ts's parser).
 */
function ruleBody(css: string, selector: string): string {
  const re = new RegExp(
    `(?:^|\\n|\\})\\s*${selector.replace(/[.[\]]/g, "\\$&")}\\s*\\{`,
  );
  const match = re.exec(css);
  if (!match) throw new Error(`selector not found: ${selector}`);
  const start = match.index + match[0].length;
  const end = css.indexOf("}", start);
  if (end === -1) throw new Error(`unterminated rule: ${selector}`);
  return css.slice(start, end);
}

describe("shell.css responsive fixes (CopyPaste-g27b.31)", () => {
  it("(a) .set-tabs wraps the 7 Settings sub-tabs instead of hiding overflow behind a scrollbar", () => {
    const body = ruleBody(SHELL_CSS, ".set-tabs");
    expect(body).toMatch(/flex-wrap:\s*wrap/);
  });

  it("(b) .about__links wraps so github/Changelog/Privacy stay within the .about pane", () => {
    const body = ruleBody(SHELL_CSS, ".about__links");
    expect(body).toMatch(/flex-wrap:\s*wrap/);
  });

  it("(c) the logs toolbar reflows (wraps) instead of pushing Refresh/Export off-screen", () => {
    const body = ruleBody(SHELL_CSS, ".logs-toolbar .srow__c");
    expect(body).toMatch(/flex-wrap:\s*wrap/);
  });

  it("(c) the log message stays visible (shrinks + wraps) instead of scrolling out of view", () => {
    const body = ruleBody(SHELL_CSS, ".logline .m");
    // Must be able to shrink below its content size (min-width:0) so the
    // flex row doesn't force horizontal overflow of `.logs`.
    expect(body).toMatch(/min-width:\s*0/);
  });

  it("(c/P3) the .lvl level badge has a fixed min-width so message text left-aligns consistently", () => {
    const body = ruleBody(SHELL_CSS, ".logline .lvl");
    expect(body).toMatch(/min-width:\s*\d/);
  });
});

describe("shell.css sidebar compact native scale (CopyPaste-7w060.5)", () => {
  it(".sb__item uses the tighter row rhythm (var(--s-3) padding, var(--s-4) gap)", () => {
    const body = ruleBody(SHELL_CSS, ".sb__item");
    expect(body).toMatch(/padding:\s*var\(--s-3\)\s*var\(--s-4\)/);
    expect(body).toMatch(/gap:\s*var\(--s-4\)/);
  });

  it(".sb__item svg icon is scaled down to var(--fs-lg) to match the compact row", () => {
    const body = ruleBody(SHELL_CSS, ".sb__item svg");
    expect(body).toMatch(/font-size:\s*var\(--fs-lg\)/);
  });

  it(".sb__item.on::before accent bar insets match the new compact padding", () => {
    const body = ruleBody(SHELL_CSS, ".sb__item.on::before");
    expect(body).toMatch(/top:\s*var\(--s-3\)/);
    expect(body).toMatch(/bottom:\s*var\(--s-3\)/);
  });

  it(".sb__foot-label ellipsizes instead of clipping under the compact footer", () => {
    const body = ruleBody(SHELL_CSS, ".sb__foot-label");
    expect(body).toMatch(/text-overflow:\s*ellipsis/);
    expect(body).toMatch(/white-space:\s*nowrap/);
    expect(body).toMatch(/min-width:\s*0/);
  });

  it(".sb__foot > .chip never shrinks so the label ellipsizes first", () => {
    const body = ruleBody(SHELL_CSS, ".sb__foot > .chip");
    expect(body).toMatch(/flex:\s*none/);
  });
});
