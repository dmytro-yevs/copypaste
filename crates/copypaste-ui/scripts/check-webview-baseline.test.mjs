import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test, { describe } from "node:test";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  loweredFor,
  missingLegacyChunks,
  postBaselineRuntime,
  postBaselineSyntax,
  sharesBaselineWithConfig,
  targetOf,
} from "./check-webview-baseline.mjs";
import { LEGACY_TARGET, MODERN_TARGET } from "./webview-baseline.mjs";

describe("which syntax each engine is missing", () => {
  // The versions come from `@babel/compat-data`, so this asserts the wiring
  // rather than a number: the older engine must need strictly more lowering.
  test("the legacy engine needs every transform the modern one does, and more", () => {
    const modern = new Set(loweredFor(MODERN_TARGET));
    const legacy = loweredFor(LEGACY_TARGET);

    assert.ok(modern.size > 0, "the modern engine should still need some lowering");
    for (const name of modern) assert.ok(legacy.includes(name), `${name} missing for legacy`);
    assert.ok(legacy.length > modern.size, "the older engine should need more");
  });

  test("async is fine on the modern engine and not on the legacy one", () => {
    const code = "async function f() { await g(); }";

    assert.deepEqual(postBaselineSyntax(code, MODERN_TARGET), []);
    assert.ok(postBaselineSyntax(code, LEGACY_TARGET).length > 0);
  });
});

describe("syntax the modern engine cannot parse", () => {
  // Chromium 74 reads `a ||= b` as `a ||` and then finds `=`, which is the
  // `Unexpected token =` run 31671766432 died on.
  test("the construct that broke API 29 is caught", () => {
    const found = postBaselineSyntax("let a = 1; a ||= 2;", MODERN_TARGET);

    assert.equal(found.length, 1);
    assert.match(found[0].what, /logical-assignment-operators/);
  });

  test("every mapped transform fires, so none can silently match nothing", () => {
    const cases = {
      "a?.b;": /optional-chaining/,
      "a ?? b;": /nullish-coalescing/,
      "a &&= b;": /logical-assignment/,
      "class C { #m() {} }": /private-methods/,
      "class C { static { x = 1; } }": /class-static-block/,
      "const n = 1_000;": /numeric-separator/,
    };

    for (const [code, expected] of Object.entries(cases)) {
      const found = postBaselineSyntax(code, MODERN_TARGET);
      assert.ok(found.length > 0, `${code} was not detected`);
      assert.match(found[0].what, expected, `${code} reported ${found[0].what}`);
    }
  });

  // Flagging these would make the gate unpassable rather than strict.
  test("syntax the modern engine already has is not flagged", () => {
    for (const code of [
      "class C { #x = 1; get x() { return this.#x; } }",
      "const { a, ...rest } = obj; const copy = { ...rest };",
      "const mod = import('./x.js'); const big = 10n;",
      "try { risky(); } catch { recover(); }",
      "const f = async (a = 1, ...rest) => { await Promise.resolve(a); return rest; };",
    ]) {
      assert.deepEqual(postBaselineSyntax(code, MODERN_TARGET), [], code);
    }
  });
});

describe("runtime APIs the engine does not have", () => {
  const CALL = "placeholder.replaceChildren(...element.cloneNode(true).childNodes)";
  const POLYFILL = "Node.prototype.replaceChildren = function () {}";

  test("an unpolyfilled call is reported as uncovered", () => {
    const found = postBaselineRuntime(CALL, MODERN_TARGET);

    assert.equal(found.length, 1);
    assert.equal(found[0].api, "ParentNode.replaceChildren");
    assert.equal(found[0].covered, false);
  });

  test("the same call is covered once the polyfill is in the build", () => {
    const found = postBaselineRuntime(`${POLYFILL}\n${CALL}`, MODERN_TARGET);

    assert.equal(found.length, 1);
    assert.equal(found[0].covered, true);
  });

  test("a build that never calls one reports nothing", () => {
    assert.deepEqual(postBaselineRuntime("const x = 1;", MODERN_TARGET), []);
    assert.deepEqual(postBaselineRuntime(POLYFILL, MODERN_TARGET), []);
  });

  // `@dnd-kit`'s collection class defines both, and matching them by name
  // reported a library's own methods as missing browser APIs.
  test("method names a library defines itself are not matched", () => {
    const dndKit = "toSorted(t){let n=[...this.entries()].sort(t)}at(r){return this.get(r)}";

    assert.deepEqual(postBaselineRuntime(dndKit, MODERN_TARGET), []);
  });
});

describe("which engine a chunk is measured against", () => {
  test("the plugin's legacy naming selects the older engine", () => {
    assert.equal(targetOf("index-Ck5oJl_4.js"), MODERN_TARGET);
    assert.equal(targetOf("index-legacy-DS5HWSd_.js"), LEGACY_TARGET);
    assert.equal(targetOf("polyfills-legacy-xDQuuDc8.js"), LEGACY_TARGET);
  });

  // `public/` is copied, not built, so both engines load whatever is in it and
  // the plugin never lowered a line of it.
  test("a script copied from public is measured against the older engine", () => {
    assert.equal(targetOf("theme-bootstrap.js", true), LEGACY_TARGET);
    assert.equal(targetOf("theme-bootstrap.js"), MODERN_TARGET);
  });

  // Object spread reached the appearance bootstrap through
  // `parseAppearanceFields.toString()`, and Chromium 53 cannot parse it: API 24
  // lost the theme, the accent and the translucency variables before first paint.
  test("the appearance bootstrap parses on the engine API 24 ships", () => {
    const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
    const source = readFileSync(path.join(root, "public", "theme-bootstrap.js"), "utf8");

    assert.deepEqual(postBaselineSyntax(source, LEGACY_TARGET, "theme-bootstrap.js"), []);
  });

  test("the config has to take its engines from the shared module", () => {
    assert.equal(
      sharesBaselineWithConfig('import { X } from "./scripts/webview-baseline.mjs";'),
      true,
    );
    assert.equal(sharesBaselineWithConfig('build: { target: "chrome74" }'), false);
  });
});

describe("what the nomodule build has to carry", () => {
  // Removing the import in `main.tsx` deletes the chunk rather than the call,
  // so the engine that needs these would load neither and say nothing.
  test("a missing polyfill chunk is named by what stops loading", () => {
    const missing = missingLegacyChunks(["index-legacy-a.js", "polyfills-legacy-b.js"]);

    assert.equal(missing.length, 1);
    assert.equal(missing[0][0], "legacyPolyfills-legacy");
    assert.match(missing[0][1], /Intl\.RelativeTimeFormat/);
  });

  test("both chunks present satisfies it", () => {
    assert.deepEqual(
      missingLegacyChunks(["polyfills-legacy-b.js", "legacyPolyfills-legacy-c.js"]),
      [],
    );
  });
});
