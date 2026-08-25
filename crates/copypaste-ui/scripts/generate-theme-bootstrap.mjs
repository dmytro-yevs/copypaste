import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { themeBootstrapSource } from "../src/lib/themeBootstrapSource.ts";

const outputPath = fileURLToPath(
  new URL("../public/theme-bootstrap.js", import.meta.url),
);
const generated = themeBootstrapSource();

if (process.argv.includes("--check")) {
  if (readFileSync(outputPath, "utf8") !== generated) {
    console.error("public/theme-bootstrap.js is stale");
    process.exitCode = 1;
  }
} else if (readFileSync(outputPath, "utf8") !== generated) {
  writeFileSync(outputPath, generated);
}
