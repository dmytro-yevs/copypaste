import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { previousFixtureVersion, syncAndroidConfig, versionCodeFor, writeVersionOverlay } from "./android-metadata.mjs";
import { projectDeepLinkConfigs, staleDeepLinkConfigs } from "./product-config.mjs";

const deepLinkFixtures = {
  tauri: `${JSON.stringify({
    plugins: {
      "deep-link": {
        desktop: { schemes: ["old-scheme"] },
        mobile: [{ scheme: ["old-scheme"], appLink: false }],
      },
    },
  }, null, 2)}\n`,
  androidManifest: `<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
  <queries>
    <intent>
      <action android:name="android.intent.action.MAIN" />
      <category android:name="android.intent.category.LAUNCHER" />
    </intent>
    <package android:name="moe.shizuku.privileged.api" />
  </queries>
  <application><activity android:name=".MainActivity">
    <intent-filter>
      <action android:name="android.intent.action.VIEW" />
      <category android:name="android.intent.category.BROWSABLE" />
      <data android:scheme="old-scheme" android:host="pair" />
    </intent-filter>
  </activity></application>
</manifest>
`,
  capability: `${JSON.stringify({
    permissions: ["core:default", "deep-link:allow-get-current", "store:default"],
  }, null, 2)}\n`,
};

test("prereleases and releases are strictly monotonic", () => {
  const versions = [
    "2.0.0-alpha.15",
    "2.0.0-alpha.16",
    "2.0.0-alpha.17",
    "2.0.0-alpha.18",
    "2.0.0-alpha.19",
    "2.0.0-alpha.20",
    "2.0.0-alpha.21",
    "2.0.0-alpha.22",
    "2.0.0-alpha.23",
    "2.0.0-alpha.24",
    "2.0.0-alpha.25",
    "2.0.0-alpha.26",
    "2.0.0-beta.0",
    "2.0.0-rc.0",
    "2.0.0",
    "2.0.1-alpha.0",
  ];
  const codes = versions.map(versionCodeFor);
  assert.deepEqual(codes, [...codes].sort((a, b) => a - b));
  assert.equal(new Set(codes).size, codes.length);
  assert.equal(versionCodeFor("2.0.0-alpha.16"), 200000016);
  assert.equal(versionCodeFor("2.0.0-alpha.17"), 200000017);
  assert.equal(versionCodeFor("2.0.0-alpha.18"), 200000018);
  assert.equal(versionCodeFor("2.0.0-alpha.19"), 200000019);
  assert.equal(versionCodeFor("2.0.0-alpha.20"), 200000020);
  assert.equal(versionCodeFor("2.0.0-alpha.21"), 200000021);
  assert.equal(versionCodeFor("2.0.0-alpha.22"), 200000022);
  assert.equal(versionCodeFor("2.0.0-alpha.23"), 200000023);
  assert.equal(versionCodeFor("2.0.0-alpha.24"), 200000024);
  assert.equal(versionCodeFor("2.0.0-alpha.25"), 200000025);
  assert.equal(versionCodeFor("2.0.0-alpha.26"), 200000026);
});

test("unsupported SemVer shapes fail closed", () => {
  for (const version of ["2.0.0-alpha", "2.0.0-preview.1", "2.0.0+local", "21.0.0"]) {
    assert.throws(() => versionCodeFor(version));
  }
});

test("upgrade fixture precedes the product version", () => {
  const cases = {
    "2.0.0-alpha.16": "2.0.0-alpha.15",
    "2.0.0-alpha.17": "2.0.0-alpha.16",
    "2.0.0-alpha.18": "2.0.0-alpha.17",
    "2.0.0-alpha.19": "2.0.0-alpha.18",
    "2.0.0-alpha.20": "2.0.0-alpha.19",
    "2.0.0-alpha.21": "2.0.0-alpha.20",
    "2.0.0-alpha.22": "2.0.0-alpha.21",
    "2.0.0-alpha.23": "2.0.0-alpha.22",
    "2.0.0-alpha.24": "2.0.0-alpha.23",
    "2.0.0-alpha.25": "2.0.0-alpha.24",
    "2.0.0-alpha.26": "2.0.0-alpha.25",
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

test("canonical scheme projects to every deep-link registration", () => {
  const projected = projectDeepLinkConfigs(deepLinkFixtures, "copy-test");
  const tauri = JSON.parse(projected.tauri);
  const capability = JSON.parse(projected.capability);

  assert.deepEqual(tauri.plugins["deep-link"].desktop.schemes, ["copy-test"]);
  assert.deepEqual(tauri.plugins["deep-link"].mobile[0], {
    scheme: ["copy-test"],
    appLink: false,
  });
  assert.match(projected.androidManifest, /android:scheme="copy-test" android:host="pair"/);
  assert.match(projected.androidManifest, /android\.intent\.action\.MAIN/);
  assert.match(projected.androidManifest, /android\.intent\.category\.LAUNCHER/);
  assert.match(projected.androidManifest, /moe\.shizuku\.privileged\.api/);
  assert.deepEqual(capability.permissions, [
    "core:default",
    "deep-link:default",
    "store:default",
  ]);
  assert.deepEqual(projectDeepLinkConfigs(projected, "copy-test"), projected);
  assert.throws(() => projectDeepLinkConfigs(projected, "CopyPaste"), /invalid deep-link scheme/);
});

test("drift check identifies each generated deep-link surface", () => {
  const projected = projectDeepLinkConfigs(deepLinkFixtures, "copypaste");
  const drift = {
    tauri: projected.tauri.replace('"copypaste"', '"wrong"'),
    androidManifest: projected.androidManifest.replace('android:scheme="copypaste"', 'android:scheme="wrong"'),
    capability: projected.capability.replace('"deep-link:default",', ""),
  };

  for (const name of Object.keys(drift)) {
    assert.deepEqual(staleDeepLinkConfigs({ ...projected, [name]: drift[name] }, "copypaste"), [name]);
  }
});
