import assert from "node:assert/strict";
import test from "node:test";

import { routeChunkErrors } from "./check-route-chunks.mjs";

const js = [
  "App-a.js",
  "App-legacy-b.js",
  "capture-a.js",
  "capture-legacy-a.js",
  "devices-a.js",
  "devices-legacy-a.js",
  "history-a.js",
  "history-legacy-a.js",
  "settings-a.js",
  "settings-legacy-a.js",
];
const references = "./capture-a.js ./devices-a.js ./history-a.js ./settings-a.js";

function errors(assets, android, source = references) {
  return routeChunkErrors({
    android,
    assets,
    indexHtml: '<link rel="stylesheet" href="./assets/index-a.css">',
    readAsset: (name) => (name.startsWith("App-") ? source : ""),
  });
}

test("accepts lazy JavaScript with one statically loaded Android stylesheet", () => {
  assert.deepEqual(errors([...js, "index-a.css"], true), []);
});

test("rejects an Android route stylesheet that can strand the import promise", () => {
  assert.ok(errors([...js, "index-a.css", "devices-a.css"], true).some((error) => error.includes("route CSS")));
});

test("accepts route-specific stylesheets in the standard browser build", () => {
  assert.deepEqual(
    errors([...js, ...["capture", "devices", "history", "settings"].map((name) => `${name}-a.css`)], false),
    [],
  );
});

test("rejects an eagerly folded route chunk", () => {
  assert.ok(errors(js.filter((name) => name !== "devices-a.js"), true).some((error) => error.startsWith("devices:")));
});
