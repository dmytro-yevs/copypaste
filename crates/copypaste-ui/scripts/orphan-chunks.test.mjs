import assert from "node:assert/strict";
import test, { describe } from "node:test";

import { dropOrphanChunks, unreferenced } from "./orphan-chunks.mjs";

function chunk(fileName, code = "") {
  return { type: "chunk", fileName, code };
}

function generate(plugin, bundle) {
  plugin.generateBundle({}, bundle);
  return Object.keys(bundle).sort();
}

describe("files nothing loads", () => {
  test("a chunk no other chunk and no html points at is one", () => {
    const chunks = [
      { name: "index-a.js", code: "import './shared-b.js'" },
      { name: "shared-b.js", code: "" },
      { name: "orphan-c.js", code: "" },
    ];

    assert.deepEqual(unreferenced(chunks, '<script src="index-a.js">'), ["orphan-c.js"]);
  });

  test("an entry named only by the html is not one", () => {
    const chunks = [{ name: "index-a.js", code: "" }];

    assert.deepEqual(unreferenced(chunks, '<script src="index-a.js">'), []);
    assert.deepEqual(unreferenced(chunks, ""), ["index-a.js"]);
  });
});

describe("dropping the chunk one output does not load", () => {
  const plugin = dropOrphanChunks(/^legacyPolyfills-/);

  // The module build's copy: the branch that imported it was replaced with
  // `false` after chunking, so the reference is gone and 66 kB of core-js
  // shipped to every device that would never fetch it.
  test("the orphan leaves the output that lost its reference", () => {
    const remaining = generate(plugin, {
      "index.html": { type: "asset", fileName: "index.html", source: '<script src="/assets/index-a.js">' },
      "assets/index-a.js": chunk("assets/index-a.js", "console.log(1)"),
      "assets/legacyPolyfills-b.js": chunk("assets/legacyPolyfills-b.js", "polyfill()"),
    });

    assert.deepEqual(remaining, ["assets/index-a.js", "index.html"]);
  });

  // Deleting a chunk something loads is the outage the condition prevents.
  test("the nomodule output keeps the copy it imports", () => {
    const remaining = generate(plugin, {
      "index.html": { type: "asset", fileName: "index.html", source: '<script src="/assets/index-legacy-a.js">' },
      "assets/index-legacy-a.js": chunk(
        "assets/index-legacy-a.js",
        'System.register(["./legacyPolyfills-legacy-b.js"],function(){})',
      ),
      "assets/legacyPolyfills-legacy-b.js": chunk("assets/legacyPolyfills-legacy-b.js", "polyfill()"),
    });

    assert.equal(remaining.length, 3);
  });

  test("an orphan the pattern does not name is left for the gate to report", () => {
    const remaining = generate(plugin, {
      "assets/index-a.js": chunk("assets/index-a.js", "console.log(1)"),
      "assets/stray-b.js": chunk("assets/stray-b.js", ""),
    });

    assert.ok(remaining.includes("assets/stray-b.js"));
  });
});
