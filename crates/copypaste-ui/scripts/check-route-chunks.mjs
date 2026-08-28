import { readFileSync, readdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

export const ROUTE_CHUNK_PREFIXES = ["capture", "devices", "history", "settings"];

function importedNames(manifest, keys) {
  return new Set((keys ?? []).map((key) => manifest[key]?.name).filter(Boolean));
}

export function routeChunkErrors({ android, assets, indexHtml, manifest }) {
  const errors = [];
  const css = assets.filter((name) => name.endsWith(".css"));
  const manifestEntries = Object.entries(manifest);
  const appEntries = manifestEntries.filter(([, entry]) => entry.name === "App");

  if (appEntries.length !== 2) errors.push(`expected modern and legacy App graph entries, found ${appEntries.length}`);
  for (const [key, app] of appEntries) {
    const dynamic = importedNames(manifest, app.dynamicImports);
    const staticImports = importedNames(manifest, app.imports);
    for (const route of ROUTE_CHUNK_PREFIXES) {
      if (!dynamic.has(route)) errors.push(`${key}: ${route} is not a dynamic import`);
      if (staticImports.has(route)) errors.push(`${key}: ${route} is also statically imported`);
    }
  }

  for (const route of ROUTE_CHUNK_PREFIXES) {
    const routeEntries = manifestEntries.filter(([, entry]) => entry.name === route);
    const modern = routeEntries.filter(([, entry]) => !entry.file.includes("-legacy-"));
    const legacy = routeEntries.filter(([, entry]) => entry.file.includes("-legacy-"));
    if (modern.length !== 1) errors.push(`${route}: expected one modern route graph entry, found ${modern.length}`);
    if (legacy.length !== 1) errors.push(`${route}: expected one legacy route graph entry, found ${legacy.length}`);
    for (const [, entry] of routeEntries) {
      const file = entry.file.replace(/^assets\//, "");
      if (!assets.includes(file)) errors.push(`${route}: manifest output is missing ${entry.file}`);
    }
    if (android) {
      if (routeEntries.some(([, entry]) => (entry.css ?? []).length > 0)) {
        errors.push(`Android: ${route} still owns split CSS`);
      }
    } else if (!modern.some(([, entry]) => (entry.css ?? []).length > 0)) {
      errors.push(`${route}: standard build lost its route stylesheet`);
    }
  }

  if (android) {
    if (css.length !== 1) errors.push(`Android: expected one static stylesheet, found ${css.length}`);
    if (css.length === 1 && !indexHtml.includes(`./assets/${css[0]}`)) {
      errors.push(`Android: index.html does not load ${css[0]}`);
    }
  } else if (css.length <= ROUTE_CHUNK_PREFIXES.length) {
    errors.push(`standard build: expected split shared and route stylesheets, found ${css.length}`);
  }

  return errors;
}

function main() {
  const root = fileURLToPath(new URL("../dist", import.meta.url));
  const assetRoot = join(root, "assets");
  const manifestPath = join(root, "route-manifest.json");
  const errors = routeChunkErrors({
    android: process.env.VITE_ANDROID_BUILD === "1",
    assets: readdirSync(assetRoot),
    indexHtml: readFileSync(join(root, "index.html"), "utf8"),
    manifest: JSON.parse(readFileSync(manifestPath, "utf8")),
  });
  if (errors.length > 0) {
    for (const error of errors) console.error(`FAIL ${error}`);
    process.exitCode = 1;
    return;
  }
  rmSync(manifestPath);
  console.log(`ok   ${process.env.VITE_ANDROID_BUILD === "1" ? "Android" : "standard"} route chunks`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main();
