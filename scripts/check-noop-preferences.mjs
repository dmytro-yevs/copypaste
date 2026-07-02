#!/usr/bin/env node
// check-noop-preferences.mjs — no-op-preference source gate (android-
// material3-redesign task 2.10): a preference exposed on `Settings` that no
// production code outside Settings/UI/storage ever READS has no behavioural
// effect — it is a no-op the UI lies about. This script flags exactly that.
//
// Method: extract every `var <name>: <Type>` property declared directly in
// `Settings.kt` (the persisted-preference surface — `val`s are excluded:
// they are typically derived/computed flags like `isRelayConfigured`, which
// are themselves consumers of other properties, not raw preferences). For
// each property, search every other `.kt` file under
// android/app/src/main/java for a `.propertyName` read, EXCLUDING files that
// are themselves Settings/UI/storage code (adapter/DI-aware: a property read
// only to persist/display it back is not a "production consumer").
//
// A property with zero qualifying usages is a NO-OP FINDING.
//
// KNOWN FINDINGS TODAY are expected and tracked by tasks.md S9.5 ("Repair
// the no-op/legacy settings"): auto_apply_synced_clip, notify_on_sensitive_skip,
// max_file_size_bytes, sync_backend. This script's job is to make that
// repair machine-checkable, not to fix it (S9 owns the fix) — S2 only
// establishes the gate. NOT wired into the blocking CI gate set in S2
// (tasks.md's Gates section does not list it as an S2-required gate); the
// owning slice (S9) is expected to run this and reach zero findings.
//
// Usage: node scripts/check-noop-preferences.mjs

import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { execSync } from "node:child_process";

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(SCRIPT_DIR, "..");
const SRC_ROOT = join(REPO_ROOT, "android/app/src/main/java");
const SETTINGS_KT = join(SRC_ROOT, "com/copypaste/android/Settings.kt");

// Adapter/DI-aware exclusion: files that are themselves the Settings/UI/
// storage layer, where a read is "displaying/persisting the preference",
// not "consuming" it.
const EXCLUDE_PATTERNS = [
  /\/Settings\.kt$/,
  /Tab\.kt$/,
  /SettingsActivity\.kt$/,
  /SettingsComponents\.kt$/,
  /SettingsScreen\.kt$/,
  /SettingsComposables?\.kt$/,
  /SettingsTypes\.kt$/,
  /SettingsUtils\.kt$/,
  /DraftSettings.*\.kt$/,
];

function listKotlinFiles() {
  return execSync(`find "${SRC_ROOT}" -name '*.kt' -type f`, { encoding: "utf8" })
    .split("\n")
    .filter(Boolean);
}

function extractPreferenceProperties() {
  const text = readFileSync(SETTINGS_KT, "utf8");
  const names = [];
  const re = /^\s{4}var\s+(\w+)\s*:/gm;
  let m;
  while ((m = re.exec(text))) names.push(m[1]);
  return names;
}

const files = listKotlinFiles();
const consumerFiles = files.filter((f) => !EXCLUDE_PATTERNS.some((p) => p.test(f)));
const properties = extractPreferenceProperties();

const noOps = [];
for (const name of properties) {
  const usagePattern = new RegExp(`\\.${name}\\b`);
  const hasConsumer = consumerFiles.some((f) => usagePattern.test(readFileSync(f, "utf8")));
  if (!hasConsumer) noOps.push(name);
}

if (noOps.length > 0) {
  console.log(`no-op-preference gate: ${noOps.length} preference(s) with no production consumer outside Settings/UI/storage:\n`);
  for (const n of noOps) console.log(`  ${n}`);
  console.log("\nSee tasks.md S9.5 ('Repair the no-op/legacy settings') — this list is that slice's acceptance target (zero findings).");
} else {
  console.log("no-op-preference gate: OK (every Settings property has a production consumer).");
}

// Not wired as blocking in S2 (see header) — always exits 0 here; S9 decides
// when to promote this to a blocking gate once it owns the repair.
process.exit(0);
