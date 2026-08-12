import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { parse as parseYaml } from "yaml";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
export const e2eRoot = path.resolve(scriptDir, "..");
export const repoRoot = path.resolve(e2eRoot, "..");
export const manifestPath = path.join(scriptDir, "dmy-45-focused-tests.json");
export const verifierCommand = "node scripts/verify-dmy-45-tests.mjs";
export const expected = JSON.parse(readFileSync(manifestPath, "utf8"));

const vitest = path.join(e2eRoot, "node_modules", "vitest", "vitest.mjs");
const vitestOptions = { cwd: e2eRoot };

export function fail(message, detail = "") {
  console.error(`DMY-45 focused selector check failed: ${message}`);
  if (detail) console.error(detail.replaceAll(e2eRoot, ".").replaceAll("\\", "/"));
  process.exit(1);
}

export function selectorFor(tests) {
  const escaped = tests.map(({ name }) =>
    name.split(" > ").at(-1).replace(/[.*+?^${}()|[\]\\]/g, "\\$&"),
  );
  return escaped.join("|");
}

export function splitAndCommands(script) {
  return String(script ?? "")
    .split("&&")
    .map((part) => part.trim())
    .filter(Boolean);
}

export function relativeTestPath(file, root = e2eRoot) {
  return path.relative(root, file).replaceAll("\\", "/");
}

function key(test) {
  return `${test.file}\n${test.name}`;
}

export function compareCollection(observed, wanted = expected) {
  const expectedKeys = new Set(wanted.map(key));
  const observedKeys = new Set(observed.map(key));
  const missing = wanted.filter((test) => !observedKeys.has(key(test)));
  const extra = observed.filter((test) => !expectedKeys.has(key(test)));
  return {
    ok: observed.length === wanted.length && missing.length === 0 && extra.length === 0,
    missing,
    extra,
    observed,
  };
}

export function checkWiring({ workflow, packageJson }) {
  const failures = [];
  const steps = workflow?.jobs?.browser?.steps ?? [];
  const focused = steps
    .map((step, index) => ({ step, index }))
    .filter(
      ({ step }) =>
        step?.["working-directory"] === "e2e" &&
        String(step?.run ?? "").trim() === "npm run test:dmy-45:browser-repeat",
    )
    .map(({ index }) => index);
  const full = steps
    .map((step, index) => ({ step, index }))
    .filter(
      ({ step }) =>
        step?.["working-directory"] === "e2e" &&
        String(step?.run ?? "").trim() === "npm test",
    )
    .map(({ index }) => index);
  const scripts = packageJson?.scripts ?? {};
  const repeat = splitAndCommands(scripts["test:dmy-45:browser-repeat"]);
  const normal = splitAndCommands(scripts.test);

  if (focused.length !== 1) failures.push("browser workflow must have one focused DMY-45 repeat step");
  if (full.length !== 1) failures.push("browser workflow must have one full npm test step");
  if (!(focused.length === 1 && full.length === 1 && focused[0] < full[0])) {
    failures.push("focused DMY-45 repeat step must run before the full npm test step");
  }
  if (repeat.length !== 3 || repeat.some((cmd) => cmd !== "npm run test:dmy-45:browser")) {
    failures.push("test:dmy-45:browser-repeat must invoke test:dmy-45:browser exactly three times");
  }
  if (scripts["test:dmy-45:verify"] !== verifierCommand) {
    failures.push("test:dmy-45:verify must run the verifier");
  }
  if (scripts["test:dmy-45"] !== `${verifierCommand} --run`) {
    failures.push("test:dmy-45 must run the verifier before focused execution");
  }
  if (
    !String(scripts["test:dmy-45:browser"] ?? "").startsWith("xvfb-run ") ||
    !String(scripts["test:dmy-45:browser"] ?? "").includes("npm run test:dmy-45")
  ) {
    failures.push("test:dmy-45:browser must wrap the verified focused script in Xvfb");
  }
  if (
    normal[0] !== verifierCommand ||
    !normal.slice(1).join(" && ").startsWith('xvfb-run -a -s "-screen 0 1280x900x24" vitest run')
  ) {
    failures.push("npm test must run the verifier before the full browser suite");
  }

  return failures;
}

export function readWiring() {
  return {
    workflow: parseYaml(
      readFileSync(path.join(repoRoot, ".github", "workflows", "browser-webkitgtk.yml"), "utf8"),
    ),
    packageJson: JSON.parse(readFileSync(path.join(e2eRoot, "package.json"), "utf8")),
  };
}

export function assertWiring(wiring = readWiring()) {
  const failures = checkWiring(wiring);
  if (failures.length) fail("browser workflow/package wiring is invalid", failures.join("\n"));
}

export function collectFocusedTests(tests = expected) {
  const selector = selectorFor(tests);
  const listed = spawnSync(
    process.execPath,
    [vitest, "list", "-t", selector, "--staticParse", "--json"],
    { ...vitestOptions, encoding: "utf8" },
  );

  if (listed.error) fail("could not start Vitest", listed.error.message);
  if (listed.status !== 0) {
    fail("Vitest collection exited nonzero", listed.stderr || listed.stdout);
  }

  try {
    return JSON.parse(listed.stdout).map(({ file, name }) => ({
      file: relativeTestPath(file),
      name,
    }));
  } catch (error) {
    fail("Vitest did not return JSON", `${error.message}\n${listed.stdout}`);
  }
}

export function assertFocusedCollection(observed = collectFocusedTests()) {
  const result = compareCollection(observed);
  if (!result.ok) {
    fail(
      `expected ${expected.length} collected tests, got ${observed.length}`,
      JSON.stringify(result, null, 2),
    );
  }
  return observed;
}

export function runFocusedTests() {
  const run = spawnSync(process.execPath, [vitest, "run", "-t", selectorFor(expected)], {
    ...vitestOptions,
    stdio: "inherit",
  });
  if (run.error) fail("could not start Vitest focused run", run.error.message);
  return run.status ?? 1;
}

function main() {
  assertWiring();
  const observed = assertFocusedCollection();
  if (!process.argv.includes("--run")) {
    console.log(`DMY-45 focused selector resolves ${observed.length} tests`);
    return 0;
  }
  return runFocusedTests();
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exit(main());
}
