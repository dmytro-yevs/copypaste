import assert from "node:assert/strict";
import test from "node:test";

import { BASELINE, configuredTarget, postBaselineSyntax } from "./check-webview-baseline.mjs";

/** The construct that actually shipped: Chromium 74 reads `a ||= b` as `a ||`
 *  followed by `=`, which is the `Unexpected token =` run 31671766432 died on. */
test("the construct that broke API 29 is caught", () => {
  const found = postBaselineSyntax("let a = 1; a ||= 2;");

  assert.equal(found.length, 1);
  assert.match(found[0].what, /logical assignment \|\|=/);
});

test("every detector fires, so none can silently match nothing", () => {
  const cases = {
    "a?.b;": /optional chaining/,
    "a?.();": /optional call/,
    "a ?? b;": /nullish coalescing/,
    "a &&= b;": /logical assignment &&=/,
    "a ??= b;": /logical assignment \?\?=/,
    "class C { #m() {} }": /private methods/,
    "class C { static { x = 1; } }": /class static blocks/,
    "const n = 1_000;": /numeric separators/,
    "await x;": /top-level await/,
  };

  for (const [code, expected] of Object.entries(cases)) {
    const found = postBaselineSyntax(code);
    assert.ok(found.length > 0, `${code} was not detected`);
    assert.match(found[0].what, expected, `${code} reported ${found[0].what}`);
  }
});

// The lowering has to leave ordinary code alone, and the baseline engine does
// have these: flagging them would make the gate unpassable rather than strict.
test("syntax the baseline engine already has is not flagged", () => {
  const within = [
    "class C { #x = 1; get x() { return this.#x; } }",
    "const f = async (a = 1, ...rest) => { await Promise.resolve(a); return rest; };",
    "const { a, ...rest } = obj; const copy = { ...rest };",
    "for (const [k, v] of Object.entries(obj)) console.log(`${k}${v}`);",
    "const mod = import('./x.js'); const big = 10n;",
    "try { risky(); } catch { recover(); }",
    "label: for (;;) break label;",
  ];

  for (const code of within) {
    assert.deepEqual(postBaselineSyntax(code), [], code);
  }
});

test("the declared target is the one the bundle is checked against", () => {
  assert.equal(configuredTarget(), BASELINE);
});
