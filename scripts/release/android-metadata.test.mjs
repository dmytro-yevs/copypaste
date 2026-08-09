import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { previousFixtureVersion, syncAndroidConfig, versionCodeFor, writeVersionOverlay } from "./android-metadata.mjs";

test("prereleases and releases are strictly monotonic", () => {
  const versions = [
    "2.0.0-alpha.15",
    "2.0.0-alpha.16",
    "2.0.0-beta.0",
    "2.0.0-rc.0",
    "2.0.0",
    "2.0.1-alpha.0",
  ];
  const codes = versions.map(versionCodeFor);
  assert.deepEqual(codes, [...codes].sort((a, b) => a - b));
  assert.equal(new Set(codes).size, codes.length);
  assert.equal(versionCodeFor("2.0.0-alpha.16"), 200000016);
});

test("unsupported SemVer shapes fail closed", () => {
  for (const version of ["2.0.0-alpha", "2.0.0-preview.1", "2.0.0+local", "21.0.0"]) {
    assert.throws(() => versionCodeFor(version));
  }
});

test("upgrade fixture precedes the product version", () => {
  const cases = {
    "2.0.0-alpha.16": "2.0.0-alpha.15",
    "2.0.0-beta.0": "2.0.0-alpha.2999",
    "2.0.0-rc.0": "2.0.0-beta.2999",
    "2.0.1-alpha.0": "2.0.0",
    "2.1.0-alpha.0": "2.0.99",
    "3.0.0-alpha.0": "2.99.99",
  };
  for (const [current, previous] of Object.entries(cases)) {
    assert.equal(previousFixtureVersion(current), previous);
    assert.ok(versionCodeFor(previous) < versionCodeFor(current));
  }
});

test("sync leaves an already-current Android config untouched", (context) => {
  const directory = mkdtempSync(join(tmpdir(), "copypaste-android-metadata-"));
  context.after(() => rmSync(directory, { recursive: true }));
  const path = join(directory, "tauri.android.conf.json");
  const product = {
    versionName: "2.0.0-alpha.16",
    releaseApplicationId: "com.copypaste.app",
    debugApplicationIdSuffix: ".debug",
  };
  writeFileSync(path, '{\n  "unrelated": [1, 2],\n  "bundle": { "android": { "minSdkVersion": 24 } }\n}\n');
  assert.equal(syncAndroidConfig(path, product), true);
  const original = readFileSync(path, "utf8");
  const config = JSON.parse(original);
  assert.deepEqual(config.unrelated, [1, 2]);
  assert.equal(config.bundle.android.minSdkVersion, 24);
  assert.match(original, /"unrelated": \[1, 2\]/);

  const changed = syncAndroidConfig(path, product);

  assert.equal(changed, false);
  assert.equal(readFileSync(path, "utf8"), original);
});

test("previous-version overlay carries versionName and versionCode", (context) => {
  const directory = mkdtempSync(join(tmpdir(), "copypaste-android-overlay-"));
  context.after(() => rmSync(directory, { recursive: true }));
  const path = join(directory, "previous.json");

  writeVersionOverlay(path, "2.0.0-alpha.15");

  assert.deepEqual(JSON.parse(readFileSync(path, "utf8")), {
    version: "2.0.0-alpha.15",
    bundle: { android: { versionCode: 200000015 } },
  });
});
