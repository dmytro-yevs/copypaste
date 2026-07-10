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
// `path:line` violation. The gate FAILS only on violations NOT in the
// baseline — i.e. it blocks new hardcoded text from today onward without
// requiring the full S13 audit/fix to land first. Regenerate the baseline
// (only when intentionally accepting new legacy debt, never to hide a new
// violation) with `--update-baseline`.
//
// Usage:
//   node scripts/check-hardcoded-text.mjs                 # gate (CI)
//   node scripts/check-hardcoded-text.mjs --update-baseline # regenerate baseline

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
      findings.push(`${relPath(file)}:${hit.line}`);
    }
  }
  return findings.sort();
}

function loadBaseline() {
  if (!existsSync(BASELINE_PATH)) return new Set();
  return new Set(
    readFileSync(BASELINE_PATH, "utf8")
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l && !l.startsWith("#")),
  );
}

const updateBaseline = process.argv.includes("--update-baseline");
const findings = scanAll();

if (updateBaseline) {
  const header = [
    "# hardcoded-text-baseline.txt — pre-existing hardcoded-user-text debt",
    "# (android-material3-redesign task 2.6). Grandfathered so the gate can be",
    "# enforced from S2 onward without blocking on the full S13 audit/fix.",
    "# Regenerate ONLY when intentionally accepting new legacy debt — never to",
    "# hide a violation introduced by redesign work. One `path:line` per line.",
    "# Generated by: node scripts/check-hardcoded-text.mjs --update-baseline",
    "",
  ].join("\n");
  writeFileSync(BASELINE_PATH, header + findings.join("\n") + "\n", "utf8");
  console.log(`Wrote ${findings.length} baseline entries to ${relPath(BASELINE_PATH)}`);
  process.exit(0);
}

const baseline = loadBaseline();
const newViolations = findings.filter((f) => !baseline.has(f));

if (newViolations.length > 0) {
  console.error(`hardcoded-text gate: ${newViolations.length} NEW hardcoded user-facing string(s) found:\n`);
  for (const v of newViolations) console.error(`  ${v}`);
  console.error(
    "\nMove these to a string resource (res/values/strings.xml + stringResource(R.string....)),\n" +
      "or if this is genuinely pre-existing debt being re-flagged, do not add it to the baseline —\n" +
      "fix it (S13 owns the full audit; new debt should not be introduced in the meantime).",
  );
  process.exit(1);
}

console.log(`hardcoded-text gate: OK (${findings.length} baselined, 0 new)`);
