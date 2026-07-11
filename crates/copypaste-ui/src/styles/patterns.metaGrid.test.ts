import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

// Regression coverage for CopyPaste-7w060.4 — expanded device card metadata
// values (e.g. "192.168.50.232", "macOS 27.0") wrapped mid-token even though
// there was room on the line, because .cfields' `minmax(max(150px, 19%), 1fr)`
// column floor left too little room for .cfield__v once the fixed 70px
// .cfield__k label, the 11px --s-5 gap, and 18px of .cfield padding (9px per
// side) were subtracted from a 150px-190px column.
//
// jsdom does not implement real layout, so — matching the existing
// text-based CSS convention in tokens.parity.test.ts / shell.responsive.test.ts
// — this asserts directly on the declared CSS rule in patterns.css.

const PATTERNS_CSS = readFileSync(
  resolve(process.cwd(), "src/styles/patterns.css"),
  "utf8",
);

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

describe(".cfields metadata grid column floor (CopyPaste-7w060.4)", () => {
  it("keeps a 70px label (.cfield__k) + 11px gap + 18px padding of overhead in mind: the column floor must leave enough room for .cfield__v to fit a single-token value (e.g. an IPv4 address) without mid-token wrapping", () => {
    const body = ruleBody(PATTERNS_CSS, ".cfields");
    const match = /minmax\(max\((\d+)px,/.exec(body);
    expect(match).not.toBeNull();
    const floorPx = Number(match![1]);
    // 70px label + 11px gap + 18px padding = 99px fixed overhead; the value
    // column needs >=110px to render "192.168.50.232" at --fs-smd/--f-mono
    // without an unnatural break, so the floor must clear ~209px.
    const fixedOverheadPx = 70 + 11 + 18;
    const minValueWidthPx = 110;
    expect(floorPx).toBeGreaterThanOrEqual(fixedOverheadPx + minValueWidthPx);
  });
});
