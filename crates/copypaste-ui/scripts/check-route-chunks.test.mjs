import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
  ROUTE_CHUNK_PREFIXES,
  RouteChunkTopologyError,
  routeChunkErrors,
  runRouteChunkGate,
} from "./check-route-chunks.mjs";

function fixture({ android, staticRoutes = false, transitiveStaticRoute = false }) {
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
  if (transitiveStaticRoute) {
    manifest.shared = {
      file: "assets/shared-route-edge-a.js",
      name: "shared-route-edge",
      imports: ["devices"],
    };
    manifest["shared-legacy"] = {
      file: "assets/shared-route-edge-legacy-a.js",
      name: "shared-route-edge",
      imports: ["devices-legacy"],
    };
    manifest.app.imports = ["shared"];
    manifest["app-legacy"].imports = ["shared-legacy"];
    assets.push("shared-route-edge-a.js", "shared-route-edge-legacy-a.js");
  }
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

function writeDist(input, manifest = JSON.stringify(input.manifest)) {
  const root = mkdtempSync(join(tmpdir(), "copypaste-route-gate-"));
  const assetRoot = join(root, "assets");
  mkdirSync(assetRoot);
  for (const asset of input.assets) writeFileSync(join(assetRoot, asset), "");
  writeFileSync(join(root, "index.html"), input.indexHtml);
  writeFileSync(join(root, "route-manifest.json"), manifest);
  return root;
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

test("rejects a route reached through a transitive static App import", () => {
  const errors = routeChunkErrors(fixture({ android: true, transitiveStaticRoute: true }));
  assert.ok(errors.some((error) => error.includes("devices is also statically imported")));
});

test("removes the temporary manifest after a reported topology error", () => {
  const input = fixture({ android: true, staticRoutes: true });
  const root = writeDist(input);
  try {
    assert.throws(
      () => runRouteChunkGate({ android: true, root }),
      (error) => error instanceof RouteChunkTopologyError,
    );
    assert.equal(existsSync(join(root, "route-manifest.json")), false);
  } finally {
    rmSync(root, { force: true, recursive: true });
  }
});

test("removes the temporary manifest after a JSON parse error", () => {
  const input = fixture({ android: true });
  const root = writeDist(input, "{");
  try {
    assert.throws(() => runRouteChunkGate({ android: true, root }), SyntaxError);
    assert.equal(existsSync(join(root, "route-manifest.json")), false);
  } finally {
    rmSync(root, { force: true, recursive: true });
  }
});

test("reports a cleanup failure only when there is no earlier gate error", () => {
  const input = fixture({ android: true });
  const root = writeDist(input);
  const cleanupError = new Error("manifest cleanup failed");
  try {
    assert.throws(
      () => runRouteChunkGate({ android: true, root, removeManifest: () => { throw cleanupError; } }),
      cleanupError,
    );
  } finally {
    rmSync(root, { force: true, recursive: true });
  }
});

test("does not mask a topology error when cleanup also fails", () => {
  const input = fixture({ android: true, staticRoutes: true });
  const root = writeDist(input);
  try {
    assert.throws(
      () => runRouteChunkGate({ android: true, root, removeManifest: () => { throw new Error("cleanup failed"); } }),
      (error) => error instanceof RouteChunkTopologyError,
    );
  } finally {
    rmSync(root, { force: true, recursive: true });
  }
});
