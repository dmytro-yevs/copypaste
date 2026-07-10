#!/usr/bin/env node
// check-l10n-completeness.mjs — localization-completeness gate (android-
// material3-redesign task 2.6/2.7, POST_UPDATE_REVIEW.md M2: "Localization
// completeness must account for non-translatable resources").
//
// Two checks, with two different enforcement levels (see rationale below):
//
// 1. translatable="false" allowlist (BLOCKING from S2 onward): every
//    `<string name="..." translatable="false">` in res/values/strings.xml
//    MUST be listed in scripts/l10n-translatable-false-allowlist.txt with a
//    reason. A new translatable="false" resource added without updating the
//    allowlist fails the gate — this is the concrete, always-checkable half
//    of M2 ("explicitly allowlist non-translatable keys").
//
// 2. EN -> UK key coverage (BLOCKING as of S13 Wave e): res/values-uk/strings.xml
//    is now complete (tasks.md "13.2 Complete values-uk/strings.xml" is done),
//    so a missing UK key is a real regression, not expected pre-S13 noise.
//    Flipped from report-only to blocking here — a repo-wide CI behavior
//    change, not a strings-content change.
//
// Usage: node scripts/check-l10n-completeness.mjs

import { readFileSync, existsSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(SCRIPT_DIR, "..");
const VALUES_DIR = join(REPO_ROOT, "android/app/src/main/res/values");
const STRINGS_EN = join(VALUES_DIR, "strings.xml");
const STRINGS_UK = join(REPO_ROOT, "android/app/src/main/res/values-uk/strings.xml");
const ALLOWLIST_PATH = join(SCRIPT_DIR, "l10n-translatable-false-allowlist.txt");

// Fix round (S6/S7/S8 file-ownership partition): each redesign slice adds its
// NEW strings to its own res/values/strings_s<N>.xml file instead of the
// shared strings.xml (to avoid merge collisions during the parallel wave —
// see e.g. strings_s7.xml's header). Android resource merging treats every
// res/values/*.xml file as one combined pool at build time, so this gate must
// scan the same pool or it silently misses every slice-owned key.
function stringsSliceFiles() {
  if (!existsSync(VALUES_DIR)) return [];
  return readdirSync(VALUES_DIR)
    .filter((f) => /^strings_s\d+\.xml$/.test(f))
    .sort()
    .map((f) => join(VALUES_DIR, f));
}

function parseStringKeys(path) {
  if (!existsSync(path)) return { keys: new Set(), nonTranslatable: new Set() };
  const xml = readFileSync(path, "utf8");
  const keys = new Set();
  const nonTranslatable = new Set();
  const re = /<string\s+name="([^"]+)"([^>]*)>/g;
  let m;
  while ((m = re.exec(xml))) {
    const name = m[1];
    const attrs = m[2];
    keys.add(name);
    if (/translatable\s*=\s*"false"/.test(attrs)) nonTranslatable.add(name);
  }
  return { keys, nonTranslatable };
}

function loadAllowlist() {
  if (!existsSync(ALLOWLIST_PATH)) return new Set();
  return new Set(
    readFileSync(ALLOWLIST_PATH, "utf8")
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l && !l.startsWith("#"))
      .map((l) => l.split(/\s+#/)[0].trim()), // "key  # reason" -> "key"
  );
}

const en = parseStringKeys(STRINGS_EN);
for (const sliceFile of stringsSliceFiles()) {
  const slice = parseStringKeys(sliceFile);
  for (const k of slice.keys) en.keys.add(k);
  for (const k of slice.nonTranslatable) en.nonTranslatable.add(k);
}
const uk = parseStringKeys(STRINGS_UK);
const allowlist = loadAllowlist();

let failed = false;

const unallowlisted = [...en.nonTranslatable].filter((k) => !allowlist.has(k)).sort();
if (unallowlisted.length > 0) {
  failed = true;
  console.error(`l10n gate: ${unallowlisted.length} translatable="false" string(s) missing from the allowlist:\n`);
  for (const k of unallowlisted) console.error(`  ${k}`);
  console.error(
    `\nAdd each to ${ALLOWLIST_PATH.replace(REPO_ROOT + "/", "")} with a "# reason" comment ` +
      "(e.g. protocol literal, machine label, app name) — android-material3-redesign M2.",
  );
} else {
  console.log(`l10n gate: translatable="false" allowlist OK (${en.nonTranslatable.size} entries, all allowlisted).`);
}

// Blocking (S13 Wave e onward): translatable EN keys with no UK counterpart.
const translatableEn = [...en.keys].filter((k) => !en.nonTranslatable.has(k));
const missingUk = translatableEn.filter((k) => !uk.keys.has(k)).sort();
if (missingUk.length > 0) {
  failed = true;
  console.error(`l10n gate: ${missingUk.length}/${translatableEn.length} translatable EN key(s) missing from values-uk/strings.xml:\n`);
  for (const k of missingUk) console.error(`  ${k}`);
} else {
  console.log(`l10n gate: values-uk/strings.xml covers all ${translatableEn.length} translatable EN keys.`);
}

process.exit(failed ? 1 : 0);
