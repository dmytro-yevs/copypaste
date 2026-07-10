#!/usr/bin/env node
// check-hardcoded-text.mjs — hardcoded-user-text gate (android-material3-
// redesign task 2.6, POST_UPDATE_REVIEW.md M1: "the hardcoded-string gate is
// too narrow" — this scans every named sink, not just Text()/
// contentDescription).
//
// A heuristic regex/line-based AST-adjacent scan (not a full Kotlin PSI
// parse — no such parser is available in this toolchain without adding a
// new build-time dependency) over every `.kt` file under
// android/app/src/main/java, flagging string LITERALS passed to known
// user-facing sinks:
//   - Text(<literal>, ...)                       (Compose)
//   - contentDescription = "<literal>"
//   - stateDescription = "<literal>"
//   - onClickLabel = "<literal>"                  (semantics + clickable)
//   - shared-component params: title/subtitle/message/label/hint/placeholder = "<literal>"
//   - Toast-like builders: makeText(ctx, "<literal>", ...)
//   - NotificationCompat.Builder setContentTitle/setContentText("<literal>")
//   - GlassToast: toastState.show("<literal>", ...)
//
// KNOWN GAP (documented, not silently swept): a full AST would also catch
// string CONCATENATION reaching a sink ("Hello " + name) and literals passed
// through multi-hop local variables. This script only catches literals
// passed directly at the call site — the review's M1 finding about indirect
// sinks is a real limitation of this heuristic pass, not fixed by it.
// CopyPaste-7vxf: this heuristic pass explicitly does NOT attempt to resolve
// a `return "Literal"` inside a helper function called at a sink (too easy to
// false-positive on non-sink helpers), nor literals reaching a sink via local
// string concatenation (already covered by the KNOWN GAP above) — both remain
// out of scope for the regex/line-based scan, not silently missed.
//
// BASELINE (pre-existing ~8% legacy hardcoded debt, design.md "Localization
// gap"): `scripts/hardcoded-text-baseline.txt` lists every currently-known
// violation. The gate FAILS only on violations NOT in the baseline — i.e. it
// blocks new hardcoded text from today onward without requiring the full S13
// audit/fix to land first.
//
// Matching is (path, literal-content) — NOT (path, line). A baseline keyed by
// line number breaks every time an unrelated edit above the entry shifts
// lines (cargo fmt, a merge, an added import) — the finding is still the same
// known FP, just at a new line, and the gate would wrongly flag it as new.
// Entries are JSON Lines (`{"path":"...","literal":"..."}`) grouped under
// `# reason: ...` comment blocks that classify *why* each literal is a false
// positive (animation label, @Deprecated message, placeholder hint, etc.) —
// the comment groups are preserved by `--update-baseline`, never rewritten.
//
// Usage:
//   node scripts/check-hardcoded-text.mjs                 # gate (CI)
//   node scripts/check-hardcoded-text.mjs --update-baseline # append new findings
//                                                          # under a fresh
//                                                          # `# reason: UNCLASSIFIED`
//                                                          # group for hand-sort —
//                                                          # never rewrites existing groups.

import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { execSync } from "node:child_process";

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(SCRIPT_DIR, "..");
const SRC_ROOT = join(REPO_ROOT, "android/app/src/main/java");
const BASELINE_PATH = join(SCRIPT_DIR, "hardcoded-text-baseline.txt");

const SINK_PATTERNS = [
  // Compose Text(<literal>...
  /\bText\(\s*"((?:[^"\\]|\\.)*)"/g,
  // named-param sinks
  /\b(?:contentDescription|stateDescription|onClickLabel|title|subtitle|message|label|hint|placeholder)\s*=\s*"((?:[^"\\]|\\.)*)"/g,
  // Toast.makeText(ctx, "...", ...)
  /\bmakeText\([^,]+,\s*"((?:[^"\\]|\\.)*)"/g,
  // NotificationCompat.Builder setContentTitle("...") / setContentText("...")
  /\bsetContentTit(?:le)\("((?:[^"\\]|\\.)*)"/g,
  /\bsetContentText\("((?:[^"\\]|\\.)*)"/g,
  // GlassToast: toastState.show("...", ...)
  /\btoastState\.show\(\s*"((?:[^"\\]|\\.)*)"/g,
];

/** A literal is "user-facing text" if it contains a letter and isn't a pure format/identifier token. */
function looksLikeUserText(literal) {
  if (literal.length === 0) return false;
  if (!/[A-Za-z]/.test(literal)) return false; // pure symbols/numbers/format strings
  if (/^%[a-zA-Z0-9$.]+$/.test(literal)) return false; // bare format specifier
  return true;
}

function listKotlinFiles() {
  const out = execSync(`find "${SRC_ROOT}" -name '*.kt' -type f`, { encoding: "utf8" });
  return out.split("\n").filter(Boolean);
}

function scanFile(path) {
  const text = readFileSync(path, "utf8");
  const lines = text.split("\n");
  const hits = [];
  lines.forEach((line, idx) => {
    // Skip comment-only lines (best-effort — does not handle block comments).
    const trimmed = line.trim();
    if (trimmed.startsWith("//") || trimmed.startsWith("*") || trimmed.startsWith("/*")) return;
    for (const pattern of SINK_PATTERNS) {
      pattern.lastIndex = 0;
      let m;
      while ((m = pattern.exec(line))) {
        const literal = m[1];
        if (looksLikeUserText(literal)) {
          hits.push({ line: idx + 1, literal });
        }
      }
    }
  });
  return hits;
}

function relPath(path) {
  return path.startsWith(REPO_ROOT + "/") ? path.slice(REPO_ROOT.length + 1) : path;
}

function scanAll() {
  const findings = [];
  for (const file of listKotlinFiles()) {
    for (const hit of scanFile(file)) {
      findings.push({ path: relPath(file), line: hit.line, literal: hit.literal });
    }
  }
  findings.sort((a, b) => (a.path === b.path ? a.line - b.line : a.path.localeCompare(b.path)));
  return findings;
}

/** Composite key for (path, literal-content) matching — line-independent. */
function findingKey(path, literal) {
  return `${path} ${literal}`;
}

/**
 * Baseline file: comment lines (`#...`) and blank lines are structural
 * (group headers) and preserved verbatim. Entry lines are JSON
 * (`{"path":"...","literal":"..."}`) so arbitrary literal content — trailing
 * spaces from concatenated strings, unicode ellipses, etc. — round-trips
 * exactly without a plain-text delimiter ambiguity.
 */
function loadBaselineLines() {
  if (!existsSync(BASELINE_PATH)) return [];
  return readFileSync(BASELINE_PATH, "utf8").split("\n");
}

function parseBaselineEntries(lines) {
  const set = new Set();
  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    try {
      const { path, literal } = JSON.parse(line);
      set.add(findingKey(path, literal));
    } catch {
      // Malformed line — ignore for matching; left untouched on disk.
    }
  }
  return set;
}

const updateBaseline = process.argv.includes("--update-baseline");
const findings = scanAll();

if (updateBaseline) {
  const existingLines = loadBaselineLines();
  const existingSet = parseBaselineEntries(existingLines);
  const newEntries = [];
  const seenNew = new Set();
  for (const f of findings) {
    const key = findingKey(f.path, f.literal);
    if (existingSet.has(key) || seenNew.has(key)) continue;
    seenNew.add(key);
    newEntries.push(f);
  }

  if (newEntries.length === 0) {
    console.log("No new findings to append — baseline already covers every current finding.");
    process.exit(0);
  }

  // Strip a single trailing blank line so the appended group doesn't leave a
  // double gap, then append a fresh UNCLASSIFIED group — every prior group is
  // left byte-for-byte untouched.
  while (existingLines.length > 0 && existingLines[existingLines.length - 1] === "") {
    existingLines.pop();
  }
  const appended = [
    ...existingLines,
    "",
    "# reason: UNCLASSIFIED — appended by --update-baseline, needs hand-sort into",
    "# the correct reason group above (or a fix) before merge.",
    ...newEntries.map((f) => JSON.stringify({ path: f.path, literal: f.literal })),
    "",
  ];
  writeFileSync(BASELINE_PATH, appended.join("\n"), "utf8");
  console.log(
    `Appended ${newEntries.length} new finding(s) to ${relPath(BASELINE_PATH)} under # reason: UNCLASSIFIED.\n` +
      "Existing reason groups were left untouched — hand-sort the new entries into the right group.",
  );
  process.exit(0);
}

const baseline = parseBaselineEntries(loadBaselineLines());
const newViolations = findings.filter((f) => !baseline.has(findingKey(f.path, f.literal)));

if (newViolations.length > 0) {
  console.error(`hardcoded-text gate: ${newViolations.length} NEW hardcoded user-facing string(s) found:\n`);
  for (const v of newViolations) console.error(`  ${v.path}:${v.line}\t${JSON.stringify(v.literal)}`);
  console.error(
    "\nMove these to a string resource (res/values/strings.xml + stringResource(R.string....)),\n" +
      "or if this is genuinely pre-existing debt being re-flagged, do not add it to the baseline —\n" +
      "fix it (S13 owns the full audit; new debt should not be introduced in the meantime).",
  );
  process.exit(1);
}

console.log(`hardcoded-text gate: OK (${findings.length} baselined, 0 new)`);
