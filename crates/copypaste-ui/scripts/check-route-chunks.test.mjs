import assert from "node:assert/strict";
import test from "node:test";

import { ROUTE_CHUNK_PREFIXES, routeChunkErrors } from "./check-route-chunks.mjs";

function fixture({ android, staticRoutes = false }) {
  const manifest = {};
  const assets = [];
  for (const route of ROUTE_CHUNK_PREFIXES) {
    for (const legacy of [false, true]) {
      const key = `${route}${legacy ? "-legacy" : ""}`;
      const file = `assets/${key}-a.js`;
      manifest[key] = {
        file,
        name: route,
        ...(!android && !legacy ? { css: [`assets/${route}-a.css`] } : {}),
      };
      assets.push(file.replace("assets/", ""));
    }
    if (!android) assets.push(`${route}-a.css`);
  }
  const routeKeys = Object.keys(manifest);
  manifest.app = {
    file: "assets/App-a.js",
    name: "App",
    imports: staticRoutes ? routeKeys.filter((key) => !key.includes("legacy")) : [],
    dynamicImports: staticRoutes ? [] : routeKeys.filter((key) => !key.includes("legacy")),
  };
  manifest["app-legacy"] = {
    file: "assets/App-legacy-a.js",
    name: "App",
    imports: staticRoutes ? routeKeys.filter((key) => key.includes("legacy")) : [],
    dynamicImports: staticRoutes ? [] : routeKeys.filter((key) => key.includes("legacy")),
  };
  assets.push("App-a.js", "App-legacy-a.js");
  if (android) assets.push("style-a.css");
  else assets.push("shared-a.css");
  return {
    android,
    assets,
    indexHtml: '<link rel="stylesheet" href="./assets/style-a.css">',
    manifest,
  };
}

test("accepts dynamic routes with one statically loaded Android stylesheet", () => {
  assert.deepEqual(routeChunkErrors(fixture({ android: true })), []);
});

test("rejects Android route CSS that can leave Vite awaiting a link event", () => {
  const input = fixture({ android: true });
  input.manifest.devices.css = ["assets/devices-a.css"];
  input.assets.push("devices-a.css");
  assert.ok(routeChunkErrors(input).some((error) => error.includes("still owns split CSS")));
});

test("accepts dynamic route stylesheets in the standard browser build", () => {
  assert.deepEqual(routeChunkErrors(fixture({ android: false })), []);
});

test("rejects an App graph that statically imports every route", () => {
  const errors = routeChunkErrors(fixture({ android: true, staticRoutes: true }));
  assert.ok(errors.some((error) => error.includes("is not a dynamic import")));
  assert.ok(errors.some((error) => error.includes("is also statically imported")));
});
