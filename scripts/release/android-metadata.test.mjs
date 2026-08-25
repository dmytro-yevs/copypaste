import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { previousFixtureVersion, syncAndroidConfig, versionCodeFor, writeVersionOverlay } from "./android-metadata.mjs";
import {
  assertNoDeepLinks,
  deepLinkSurfaces,
} from "./product-config.mjs";

const noDeepLinkFixtures = {
  cargoMetadata: {
    metadata: { copypaste: {} },
    packages: [{ name: "copypaste-ui", dependencies: [] }],
  },
  uiPackage: { dependencies: {} },
  uiLock: { packages: { "": { dependencies: {} } } },
  tauri: '{ "bundle": { "active": true } }\n',
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
      <action android:name="android.intent.action.MAIN" />
      <category android:name="android.intent.category.LAUNCHER" />
    </intent-filter>
  </activity></application>
</manifest>
`,
  capability: '{ "permissions": ["core:default", "store:default"] }\n',
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
    "2.0.0-alpha.27",
    "2.0.0-alpha.28",
    "2.0.0-alpha.29",
    "2.0.0-alpha.30",
    "2.0.0-alpha.31",
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
  assert.equal(versionCodeFor("2.0.0-alpha.27"), 200000027);
  assert.equal(versionCodeFor("2.0.0-alpha.28"), 200000028);
  assert.equal(versionCodeFor("2.0.0-alpha.29"), 200000029);
  assert.equal(versionCodeFor("2.0.0-alpha.30"), 200000030);
  assert.equal(versionCodeFor("2.0.0-alpha.31"), 200000031);
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
    "2.0.0-alpha.27": "2.0.0-alpha.26",
    "2.0.0-alpha.28": "2.0.0-alpha.27",
    "2.0.0-alpha.29": "2.0.0-alpha.28",
    "2.0.0-alpha.30": "2.0.0-alpha.29",
    "2.0.0-alpha.31": "2.0.0-alpha.30",
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

test("normal launch and Android queries are not deep links", () => {
  assert.deepEqual(deepLinkSurfaces(noDeepLinkFixtures), []);
  assert.doesNotThrow(() => assertNoDeepLinks(noDeepLinkFixtures));
  assert.match(noDeepLinkFixtures.androidManifest, /android\.intent\.action\.MAIN/);
  assert.match(noDeepLinkFixtures.androidManifest, /android\.intent\.category\.LAUNCHER/);
  assert.match(noDeepLinkFixtures.androidManifest, /moe\.shizuku\.privileged\.api/);
});

test("every deep-link registration owner fails closed", () => {
  const cases = [
    ["Cargo product metadata", (fixture) => { fixture.cargoMetadata.metadata.copypaste["deep-link-scheme"] = "copy-test"; }],
    ["Rust plugin dependency", (fixture) => { fixture.cargoMetadata.packages[0].dependencies.push({ name: "tauri-plugin-deep-link" }); }],
    ["JavaScript plugin dependency", (fixture) => { fixture.uiPackage.dependencies["@tauri-apps/plugin-deep-link"] = "2"; }],
    ["JavaScript plugin lock entry", (fixture) => { fixture.uiLock.packages["node_modules/@tauri-apps/plugin-deep-link"] = {}; }],
    ["Tauri plugin configuration", (fixture) => { fixture.tauri = '{ "plugins": { "deep-link": {} } }'; }],
    ["Tauri capability grant", (fixture) => { fixture.capability = '{ "permissions": ["deep-link:default"] }'; }],
    ["Android deep-link intent filter", (fixture) => {
      fixture.androidManifest = fixture.androidManifest.replace(
        "</activity>",
        '<intent-filter><action android:name="android.intent.action.VIEW" /><data android:scheme="copy-test" /></intent-filter></activity>',
      );
    }],
    ["Android generated plugin block", (fixture) => {
      fixture.androidManifest = fixture.androidManifest.replace(
        "</activity>",
        "<!-- DEEP LINK PLUGIN. AUTO-GENERATED. DO NOT REMOVE. --></activity>",
      );
    }],
  ];

  for (const [surface, mutate] of cases) {
    const fixture = structuredClone(noDeepLinkFixtures);
    mutate(fixture);
    assert.deepEqual(deepLinkSurfaces(fixture), [surface]);
    assert.throws(() => assertNoDeepLinks(fixture), /deep links are not a product surface/);
  }
});

test("Android XML comments cannot register a deep link", () => {
  const fixture = structuredClone(noDeepLinkFixtures);
  fixture.androidManifest = fixture.androidManifest.replace(
    "</activity>",
    `<!--
      <intent-filter>
        <action android:name="android.intent.action.VIEW" />
        <data android:scheme="copy-test" />
      </intent-filter>
    --></activity>`,
  );

  assert.deepEqual(deepLinkSurfaces(fixture), []);
  assert.doesNotThrow(() => assertNoDeepLinks(fixture));
});
