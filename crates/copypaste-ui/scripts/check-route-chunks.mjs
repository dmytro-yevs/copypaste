import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

export const ROUTE_CHUNK_PREFIXES = ["capture", "devices", "history", "settings"];

export function routeChunkErrors({ android, assets, indexHtml, readAsset }) {
  const errors = [];
  const css = assets.filter((name) => name.endsWith(".css"));

  for (const prefix of ROUTE_CHUNK_PREFIXES) {
    const modern = assets.filter(
      (name) => name.startsWith(`${prefix}-`) && name.endsWith(".js") && !name.includes("-legacy-"),
    );
    const legacy = assets.filter(
      (name) => name.startsWith(`${prefix}-legacy-`) && name.endsWith(".js"),
    );
    if (modern.length !== 1) errors.push(`${prefix}: expected one modern lazy route chunk, found ${modern.length}`);
    if (legacy.length !== 1) errors.push(`${prefix}: expected one legacy lazy route chunk, found ${legacy.length}`);
  }

  const routeCss = css.filter((name) => ROUTE_CHUNK_PREFIXES.some((prefix) => name.startsWith(`${prefix}-`)));
  if (android) {
    if (css.length !== 1) errors.push(`Android: expected one static stylesheet, found ${css.length}`);
    if (routeCss.length !== 0) errors.push(`Android: lazy route CSS remained split: ${routeCss.join(", ")}`);
    if (css.length === 1 && !indexHtml.includes(`./assets/${css[0]}`)) {
      errors.push(`Android: index.html does not load ${css[0]}`);
    }
  } else {
    for (const prefix of ROUTE_CHUNK_PREFIXES) {
      if (!routeCss.some((name) => name.startsWith(`${prefix}-`))) {
        errors.push(`${prefix}: standard build lost its route stylesheet`);
      }
    }
  }

  const appChunks = assets.filter((name) => name.startsWith("App-") && name.endsWith(".js"));
  const appSource = appChunks.map((name) => readAsset(name)).join("\n");
  for (const prefix of ROUTE_CHUNK_PREFIXES) {
    if (!appSource.includes(`./${prefix}-`)) errors.push(`${prefix}: App chunks do not reference the lazy route`);
  }
  if (android && routeCss.some((name) => appSource.includes(`./${name}`))) {
    errors.push("Android: App route loader still waits on a split stylesheet");
  }

  return errors;
}

function main() {
  const root = fileURLToPath(new URL("../dist", import.meta.url));
  const assetRoot = join(root, "assets");
  const errors = routeChunkErrors({
    android: process.env.VITE_ANDROID_BUILD === "1",
    assets: readdirSync(assetRoot),
    indexHtml: readFileSync(join(root, "index.html"), "utf8"),
    readAsset: (name) => readFileSync(join(assetRoot, name), "utf8"),
  });
  if (errors.length > 0) {
    for (const error of errors) console.error(`FAIL ${error}`);
    process.exitCode = 1;
    return;
  }
  console.log(`ok   ${process.env.VITE_ANDROID_BUILD === "1" ? "Android" : "standard"} route chunks`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main();
