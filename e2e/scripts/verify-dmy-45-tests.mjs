import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(scriptDir, "..");
const manifestPath = path.join(scriptDir, "dmy-45-focused-tests.json");
const expected = JSON.parse(readFileSync(manifestPath, "utf8"));
const vitest = path.join(root, "node_modules", "vitest", "vitest.mjs");
const vitestOptions = { cwd: root };

function fail(message, detail = "") {
  console.error(`DMY-45 focused selector check failed: ${message}`);
  if (detail) console.error(detail.replaceAll(root, ".").replaceAll("\\", "/"));
  process.exit(1);
}

function selectorFor(tests) {
  const escaped = tests.map(({ name }) =>
    name.split(" > ").at(-1).replace(/[.*+?^${}()|[\]\\]/g, "\\$&"),
  );
  return escaped.join("|");
}

function relativeTestPath(file) {
  return path.relative(root, file).replaceAll("\\", "/");
}

function key(test) {
  return `${test.file}\n${test.name}`;
}

const selector = selectorFor(expected);
const listed = spawnSync(
  process.execPath,
  [vitest, "list", "-t", selector, "--staticParse", "--json"],
  { ...vitestOptions, encoding: "utf8" },
);

if (listed.error) fail("could not start Vitest", listed.error.message);
if (listed.status !== 0) {
  fail("Vitest collection exited nonzero", listed.stderr || listed.stdout);
}

let observed;
try {
  observed = JSON.parse(listed.stdout).map(({ file, name }) => ({
    file: relativeTestPath(file),
    name,
  }));
} catch (error) {
  fail("Vitest did not return JSON", `${error.message}\n${listed.stdout}`);
}

const expectedKeys = new Set(expected.map(key));
const observedKeys = new Set(observed.map(key));
const missing = expected.filter((test) => !observedKeys.has(key(test)));
const extra = observed.filter((test) => !expectedKeys.has(key(test)));

if (observed.length !== expected.length || missing.length || extra.length) {
  fail(
    `expected ${expected.length} collected tests, got ${observed.length}`,
    JSON.stringify({ missing, extra, observed }, null, 2),
  );
}

if (!process.argv.includes("--run")) {
  console.log(`DMY-45 focused selector resolves ${observed.length} tests`);
  process.exit(0);
}

const run = spawnSync(process.execPath, [vitest, "run", "-t", selector], {
  ...vitestOptions,
  stdio: "inherit",
});
if (run.error) fail("could not start Vitest focused run", run.error.message);
process.exit(run.status ?? 1);
