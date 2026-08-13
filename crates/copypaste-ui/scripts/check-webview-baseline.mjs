/**
 * The shipped bundle must parse in the oldest WebView the Android matrix runs.
 *
 * `minSdk = 24`, and the emulator's WebView is the one pinned into its system
 * image rather than a Play-updated one, so API 29 runs Chromium 74. Building at
 * `es2022` left logical assignment in the bundle; Chromium 74 reads `a ||= b`
 * as `a ||` followed by `=`, and run 31671766432 lost every API 29 assertion —
 * UI, storage and cloud — to that one `Uncaught SyntaxError: Unexpected token
 * =` before a line of the app ran.
 *
 * Detection is per construct rather than per ECMAScript year because the two
 * do not line up: Chromium 74 has dynamic `import()` and BigInt, which are
 * ES2020, and lacks optional chaining, which is also ES2020. Parsing is
 * `@babel/parser`'s; only the walk is ours. `--self-test` proves each detector
 * fires, because a detector that silently matches nothing is the failure mode
 * this file exists to prevent.
 */
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { parse } from "@babel/parser";
import _traverse from "@babel/traverse";

const traverse = _traverse.default ?? _traverse;
const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");

export const BASELINE = "chrome74";

/** Each entry is a construct the baseline engine cannot parse, and the Chromium
 *  that introduced it. Private *fields* are absent deliberately: they landed in
 *  74 itself, so `#x = 1` is within the baseline while `#x() {}` is not. */
const AFTER_BASELINE = [
  ["OptionalMemberExpression", "optional chaining (Chromium 80)"],
  ["OptionalCallExpression", "optional call (Chromium 80)"],
  ["ClassPrivateMethod", "private methods (Chromium 84)"],
  ["StaticBlock", "class static blocks (Chromium 94)"],
];

const LOGICAL_ASSIGNMENT = new Set(["||=", "&&=", "??="]);

export function postBaselineSyntax(code, filename = "<input>") {
  const found = [];
  // Babel rejects a visitor that returns anything, so every one of these ends
  // in a statement rather than an expression body.
  const note = (node, what) => {
    found.push({ file: filename, line: node.loc?.start.line ?? 0, what });
  };

  const ast = parse(code, {
    sourceType: "unambiguous",
    allowAwaitOutsideFunction: true,
    errorRecovery: false,
  });

  const visitor = {
    LogicalExpression(nodePath) {
      if (nodePath.node.operator === "??") note(nodePath.node, "nullish coalescing (Chromium 80)");
    },
    AssignmentExpression(nodePath) {
      if (LOGICAL_ASSIGNMENT.has(nodePath.node.operator)) {
        note(nodePath.node, `logical assignment ${nodePath.node.operator} (Chromium 85)`);
      }
    },
    NumericLiteral(nodePath) {
      if ((nodePath.node.extra?.raw ?? "").includes("_")) {
        note(nodePath.node, "numeric separators (Chromium 75)");
      }
    },
    AwaitExpression(nodePath) {
      if (!nodePath.getFunctionParent()) note(nodePath.node, "top-level await (Chromium 89)");
    },
  };
  for (const [type, what] of AFTER_BASELINE) {
    visitor[type] = (nodePath) => {
      note(nodePath.node, what);
    };
  }

  traverse(ast, visitor);
  return found;
}

/** The config is the only thing that makes the emitted bundle baseline-clean,
 *  so it is read rather than trusted. */
export function configuredTarget() {
  const config = readFileSync(path.join(root, "vite.config.ts"), "utf8");
  return /\btarget:\s*"([^"]+)"/.exec(config)?.[1];
}

function bundles() {
  const assets = path.join(root, "dist", "assets");
  return readdirSync(assets)
    .filter((name) => name.endsWith(".js"))
    .map((name) => path.join(assets, name));
}

function main() {
  const target = configuredTarget();
  if (target !== BASELINE) {
    console.error(`FAIL vite.config.ts builds for ${target ?? "no declared target"}, not ${BASELINE}`);
    return 1;
  }

  let files = [];
  try {
    files = bundles();
  } catch {
    console.error("FAIL dist/assets holds no built bundle; run vite build first");
    return 1;
  }
  if (files.length === 0) {
    console.error("FAIL dist/assets holds no built bundle; run vite build first");
    return 1;
  }

  const found = files.flatMap((file) =>
    postBaselineSyntax(readFileSync(file, "utf8"), path.basename(file)),
  );
  for (const { file, line, what } of found.slice(0, 20)) {
    console.error(`FAIL ${file}:${line} uses ${what}`);
  }
  if (found.length) {
    console.error(`FAIL ${found.length} construct(s) the ${BASELINE} WebView cannot parse`);
    return 1;
  }
  console.log(`ok   ${files.length} bundle(s) parse for the ${BASELINE} WebView baseline`);
  return 0;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exit(main());
