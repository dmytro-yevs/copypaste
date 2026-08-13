import assert from "node:assert/strict";
import test, { describe } from "node:test";

import {
  BASELINE,
  configuredTarget,
  postBaselineRuntime,
  postBaselineSyntax,
} from "./check-webview-baseline.mjs";

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

// Syntax lowering cannot add a method, and the pinned `lib` cannot see a
// dependency's call, so this half is the only thing standing between the two.
describe("runtime APIs the baseline engine does not have", () => {
  const CALL = "placeholder.replaceChildren(...element.cloneNode(true).childNodes)";
  const POLYFILL = "Node.prototype.replaceChildren = function () {}";

  test("an unpolyfilled call is reported as uncovered", () => {
    const found = postBaselineRuntime(CALL);

    assert.equal(found.length, 1);
    assert.equal(found[0].api, "ParentNode.replaceChildren");
    assert.equal(found[0].covered, false);
  });

  test("the same call is covered once the polyfill is in the bundle", () => {
    const found = postBaselineRuntime(`${POLYFILL}\n${CALL}`);

    assert.equal(found.length, 1);
    assert.equal(found[0].covered, true);
    assert.equal(found[0].polyfill, "@ungap/replace-children");
  });

  // A polyfill nothing calls is not a finding, and a bundle that reaches none
  // of these must not be reported as covered-by-luck.
  test("a bundle that never calls one reports nothing", () => {
    assert.deepEqual(postBaselineRuntime("const x = 1;"), []);
    assert.deepEqual(postBaselineRuntime(POLYFILL), []);
  });

  // The false positive that made this list short: `@dnd-kit`'s own collection
  // class defines `at` and `toSorted`, and matching those by name reported a
  // library's methods as missing browser APIs.
  test("method names a library defines itself are not matched", () => {
    const dndKit = "toSorted(t){let n=[...this.entries()].sort(t)}at(r){return this.get(r)}";

    assert.deepEqual(postBaselineRuntime(dndKit), []);
  });
});
