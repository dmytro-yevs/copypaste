import { createHash } from "node:crypto";
import { execFile } from "node:child_process";
import { link, mkdir, mkdtemp, readFile, rm, symlink, unlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

import { validateEvidence } from "./native-parity-gate.mjs";

const COMMIT = "0123456789abcdef0123456789abcdef01234567";
const RUN_ID = "123456789";
const run = promisify(execFile);
const WINDOWS_STATES = new Map([
  ["history", { state: "populated", name: "Clipboard history", png: "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAFklEQVR4nGO8o6bGwMDAxMDAoHzzJgARtAMB3qLZtwAAAABJRU5ErkJggg==" }],
  ["capture", { state: "service-capture-status", name: "Background capture", png: "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEUlEQVR4nGMUW+zFwMDAAKEAEOcCCBlQdmcAAAAASUVORK5CYII=" }],
  ["devices", { state: "ready-to-pair", name: "Ready to pair", png: "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAFklEQVR4nGNUTX7NwMDAxMDAcGuOCAAUogMBx1ZqEgAAAABJRU5ErkJggg==" }],
  ["settings-and-service", { state: "appearance", name: "Theme", png: "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAFklEQVR4nGOcbPyKgYGBiYGBIX2mNgAWpQLf2/uWLgAAAABJRU5ErkJggg==" }],
  ["settings-and-service/updater-configured", { feature: "settings-and-service", state: "updater-configured", name: "Check for updates", png: "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEUlEQVR4nGP8z8DAwMDAAAANHQEDasKb6QAAAABJRU5ErkJggg==" }],
  ["cloud-account", { state: "not-configured", name: "Not configured", png: "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAFklEQVR4nGPkmLiJgYGBiYGBQWz9XwARoQMR3PtkxgAAAABJRU5ErkJggg==" }],
]);
const WINDOWS_UNCONFIGURED_UPDATER = {
  feature: "settings-and-service",
  state: "updater-unconfigured",
  name: "Updates aren't configured in this build.",
  png: "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEUlEQVR4nGP8z8DAwMDAAAANHQEDasKb6QAAAABJRU5ErkJggg==",
};
const PNG = Buffer.from(WINDOWS_STATES.get("history").png, "base64");

const RECEIPT_VALUES = {
  macos: {
    environment: "hosted-runner",
    scenario: { name: "native-launch", elapsed_ms: 10, budget_ms: 3000 },
    assertions: [
      "installed app launched",
      "native accessibility tree is non-empty",
      "native accessibility surface exposes a menu bar and named elements",
    ],
  },
  android: {
    environment: "emulator",
    scenario: { name: "release-webview-ready", elapsed_ms: 10, budget_ms: 115000 },
    assertions: [
      "signed release app launched",
      "WebView accessibility content painted",
      "release smoke assertions passed",
    ],
  },
  windows: {
    environment: "hosted-runner",
    scenario: { name: "windows-installed-release", elapsed_ms: 10, budget_ms: 30000 },
    assertions: [
      "installer integrity passed",
      "installed app launched",
      "installed sidecar launched",
      "named-pipe and clipboard passed",
      "update feed contract matched signing mode",
      "in-place update passed",
      "feature-specific UI states captured",
      "screenshot protection restored",
      "uninstall passed",
    ],
  },
};

async function fixture(root, platform, overrides = {}) {
  const { windowsUpdaterState = "updater-configured", ...receiptOverrides } = overrides;
  const directory = path.join(root, platform);
  const artifacts = [];
  const kinds = platform === "windows"
    ? ["test-log", "measurement"]
    : ["screenshot", "accessibility", "measurement"];
  await mkdir(directory, { recursive: true });

  if (platform === "windows") {
    const states = [];
    const windowsStates = new Map(WINDOWS_STATES);
    if (windowsUpdaterState === "updater-unconfigured") {
      windowsStates.delete("settings-and-service/updater-configured");
      windowsStates.set("settings-and-service/updater-unconfigured", WINDOWS_UNCONFIGURED_UPDATER);
    }
    for (const [evidenceDirectory, expected] of windowsStates) {
      const feature = expected.feature ?? evidenceDirectory;
      const featureDirectory = path.join(directory, evidenceDirectory);
      await mkdir(featureDirectory, { recursive: true });
      const screenshotPath = `${evidenceDirectory}/screenshot.png`;
      const accessibilityPath = `${evidenceDirectory}/accessibility.json`;
      const screenshot = Buffer.from(expected.png, "base64");
      const accessibility = `${JSON.stringify({
        schema_version: 2,
        feature,
        state: expected.state,
        expected_name: expected.name,
        window: {
          handle: 1,
          foreground: true,
          visible: true,
          minimized: false,
          capture_allowed: true,
          display_affinity: 0,
          capture_bounds: { kind: "client", x: 0, y: 0, width: 1, height: 1 },
        },
        node_read: { complete: true, read: 1, retried: [] },
        nodes: [{ name: expected.name, enabled: true, offscreen: false, bounds: { x: 0, y: 0, width: 1, height: 1 } }],
      }, null, 2)}\n`;
      await writeFile(path.join(directory, screenshotPath), screenshot);
      await writeFile(path.join(directory, accessibilityPath), accessibility);
      states.push({
        feature,
        state: expected.state,
        expected_name: expected.name,
        screenshot: {
          path: screenshotPath,
          sha256: createHash("sha256").update(screenshot).digest("hex"),
          bytes: screenshot.length,
        },
        accessibility: {
          path: accessibilityPath,
          sha256: createHash("sha256").update(accessibility).digest("hex"),
          bytes: Buffer.byteLength(accessibility),
        },
      });
      if (!expected.state.startsWith("updater-")) {
        artifacts.push(
          { kind: "screenshot", ...states.at(-1).screenshot },
          { kind: "accessibility", ...states.at(-1).accessibility },
        );
      }
    }
    const manifest = `${JSON.stringify({ schema_version: 1, states }, null, 2)}\n`;
    await writeFile(path.join(directory, "feature-states.json"), manifest);
    artifacts.push({
      kind: "feature-evidence",
      path: "feature-states.json",
      sha256: createHash("sha256").update(manifest).digest("hex"),
      bytes: Buffer.byteLength(manifest),
    });
  }

  for (const kind of kinds) {
    const name = `${kind}.txt`;
    const contents = `${platform}-${kind}\n`;
    await writeFile(path.join(directory, name), contents);
    artifacts.push({
      kind,
      path: name,
      sha256: createHash("sha256").update(contents).digest("hex"),
      bytes: Buffer.byteLength(contents),
    });
  }

  const receipt = {
    schema_version: 1,
    platform,
    environment: RECEIPT_VALUES[platform].environment,
    os_version: "fixture-os",
    architecture: "fixture-arch",
    source: { commit: COMMIT, run_id: RUN_ID },
    scenario: RECEIPT_VALUES[platform].scenario,
    assertions: RECEIPT_VALUES[platform].assertions,
    artifacts,
    ...receiptOverrides,
  };
  const receiptPath = path.join(directory, "native-evidence.json");
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  return receiptPath;
}

async function withRoot(run) {
  const root = await mkdtemp(path.join(tmpdir(), "native-parity-"));
  try {
    await run(root);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

test("accepts alpha.29 macOS, emulator Android, and Windows release evidence", () => withRoot(async (root) => {
  const evidence = await Promise.all([
    fixture(root, "macos"),
    fixture(root, "android"),
    fixture(root, "windows"),
  ]);
  const receipts = await validateEvidence({
    commit: COMMIT,
    evidence,
    required: new Set(["macos", "android", "windows"]),
    runId: RUN_ID,
  });
  assert.equal(receipts.length, 3);
}));

test("accepts unconfigured updater evidence only when the unsigned workflow requests it", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "windows", { windowsUpdaterState: "updater-unconfigured" });
  const options = {
    commit: COMMIT,
    evidence: [receiptPath],
    required: new Set(["windows"]),
    runId: RUN_ID,
  };
  await assert.rejects(validateEvidence(options), /updater-unconfigured/);
  const receipts = await validateEvidence({ ...options, windowsUpdaterState: "updater-unconfigured" });
  assert.equal(receipts.length, 1);
}));

test("fails closed when a required platform is absent", () => withRoot(async (root) => {
  const evidence = [await fixture(root, "macos")];
  await assert.rejects(
    validateEvidence({
      commit: COMMIT,
      evidence,
      required: new Set(["macos", "android"]),
      runId: RUN_ID,
    }),
    /missing required evidence for android/,
  );
}));

test("fails closed when Windows release evidence is absent", () => withRoot(async (root) => {
  const evidence = [await fixture(root, "macos"), await fixture(root, "android")];
  await assert.rejects(
    validateEvidence({
      commit: COMMIT,
      evidence,
      required: new Set(["macos", "android", "windows"]),
      runId: RUN_ID,
    }),
    /missing required evidence for windows/,
  );
}));

test("rejects a missing Windows updater state", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "windows");
  const manifestPath = path.join(path.dirname(receiptPath), "feature-states.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  manifest.states = manifest.states.filter((state) => state.state !== "updater-configured");
  const contents = `${JSON.stringify(manifest, null, 2)}\n`;
  await writeFile(manifestPath, contents);
  const receipt = JSON.parse(await readFile(receiptPath, "utf8"));
  const index = receipt.artifacts.find((artifact) => artifact.kind === "feature-evidence");
  index.sha256 = createHash("sha256").update(contents).digest("hex");
  index.bytes = Buffer.byteLength(contents);
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  await assert.rejects(
    validateEvidence({ commit: COMMIT, evidence: [receiptPath], required: new Set(["windows"]), runId: RUN_ID }),
    /exact Windows feature state set/,
  );
}));

test("rejects an unknown Windows feature state", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "windows");
  const manifestPath = path.join(path.dirname(receiptPath), "feature-states.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  const updater = manifest.states.find((state) => state.state === "updater-configured");
  updater.state = "updater-unknown";
  const contents = `${JSON.stringify(manifest, null, 2)}\n`;
  await writeFile(manifestPath, contents);
  const receipt = JSON.parse(await readFile(receiptPath, "utf8"));
  const index = receipt.artifacts.find((artifact) => artifact.kind === "feature-evidence");
  index.sha256 = createHash("sha256").update(contents).digest("hex");
  index.bytes = Buffer.byteLength(contents);
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  await assert.rejects(
    validateEvidence({ commit: COMMIT, evidence: [receiptPath], required: new Set(["windows"]), runId: RUN_ID }),
    /unknown or wrong Windows feature state settings-and-service\/updater-unknown/,
  );
}));

test("rejects a duplicate Windows feature state", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "windows");
  const manifestPath = path.join(path.dirname(receiptPath), "feature-states.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  manifest.states.at(-1).feature = "settings-and-service";
  manifest.states.at(-1).state = "updater-configured";
  const contents = `${JSON.stringify(manifest, null, 2)}\n`;
  await writeFile(manifestPath, contents);
  const receipt = JSON.parse(await readFile(receiptPath, "utf8"));
  const index = receipt.artifacts.find((artifact) => artifact.kind === "feature-evidence");
  index.sha256 = createHash("sha256").update(contents).digest("hex");
  index.bytes = Buffer.byteLength(contents);
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  await assert.rejects(
    validateEvidence({ commit: COMMIT, evidence: [receiptPath], required: new Set(["windows"]), runId: RUN_ID }),
    /invalid or duplicate Windows feature state/,
  );
}));

test("rejects an offscreen Windows state marker", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "windows");
  const manifestPath = path.join(path.dirname(receiptPath), "feature-states.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  const state = manifest.states[0];
  const accessibilityPath = path.join(path.dirname(receiptPath), state.accessibility.path);
  const accessibility = JSON.parse(await readFile(accessibilityPath, "utf8"));
  accessibility.nodes[0].offscreen = true;
  const accessibilityContents = `${JSON.stringify(accessibility, null, 2)}\n`;
  await writeFile(accessibilityPath, accessibilityContents);
  state.accessibility.sha256 = createHash("sha256").update(accessibilityContents).digest("hex");
  state.accessibility.bytes = Buffer.byteLength(accessibilityContents);
  const manifestContents = `${JSON.stringify(manifest, null, 2)}\n`;
  await writeFile(manifestPath, manifestContents);
  const receipt = JSON.parse(await readFile(receiptPath, "utf8"));
  Object.assign(receipt.artifacts.find((artifact) => artifact.path === state.accessibility.path), state.accessibility);
  const index = receipt.artifacts.find((artifact) => artifact.kind === "feature-evidence");
  index.sha256 = createHash("sha256").update(manifestContents).digest("hex");
  index.bytes = Buffer.byteLength(manifestContents);
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  await assert.rejects(
    validateEvidence({ commit: COMMIT, evidence: [receiptPath], required: new Set(["windows"]), runId: RUN_ID }),
    /does not prove its expected UI state/,
  );
}));

test("rejects reused screenshot identity across Windows states", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "windows");
  const manifestPath = path.join(path.dirname(receiptPath), "feature-states.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  const source = manifest.states[0].screenshot;
  const target = manifest.states[1].screenshot;
  const bytes = await readFile(path.join(path.dirname(receiptPath), source.path));
  await writeFile(path.join(path.dirname(receiptPath), target.path), bytes);
  target.sha256 = source.sha256;
  target.bytes = source.bytes;
  const manifestContents = `${JSON.stringify(manifest, null, 2)}\n`;
  await writeFile(manifestPath, manifestContents);
  const receipt = JSON.parse(await readFile(receiptPath, "utf8"));
  Object.assign(receipt.artifacts.find((artifact) => artifact.path === target.path), target);
  const index = receipt.artifacts.find((artifact) => artifact.kind === "feature-evidence");
  index.sha256 = createHash("sha256").update(manifestContents).digest("hex");
  index.bytes = Buffer.byteLength(manifestContents);
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  await assert.rejects(
    validateEvidence({ commit: COMMIT, evidence: [receiptPath], required: new Set(["windows"]), runId: RUN_ID }),
    /reuses a screenshot identity/,
  );
}));

test("rejects a Windows capture without foreground HWND proof", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "windows");
  const manifestPath = path.join(path.dirname(receiptPath), "feature-states.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  const state = manifest.states[0];
  const accessibilityPath = path.join(path.dirname(receiptPath), state.accessibility.path);
  const accessibility = JSON.parse(await readFile(accessibilityPath, "utf8"));
  accessibility.window.foreground = false;
  const accessibilityContents = `${JSON.stringify(accessibility, null, 2)}\n`;
  await writeFile(accessibilityPath, accessibilityContents);
  state.accessibility.sha256 = createHash("sha256").update(accessibilityContents).digest("hex");
  state.accessibility.bytes = Buffer.byteLength(accessibilityContents);
  const manifestContents = `${JSON.stringify(manifest, null, 2)}\n`;
  await writeFile(manifestPath, manifestContents);
  const receipt = JSON.parse(await readFile(receiptPath, "utf8"));
  Object.assign(receipt.artifacts.find((artifact) => artifact.path === state.accessibility.path), state.accessibility);
  const index = receipt.artifacts.find((artifact) => artifact.kind === "feature-evidence");
  index.sha256 = createHash("sha256").update(manifestContents).digest("hex");
  index.bytes = Buffer.byteLength(manifestContents);
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  await assert.rejects(
    validateEvidence({ commit: COMMIT, evidence: [receiptPath], required: new Set(["windows"]), runId: RUN_ID }),
    /accessibility describes the wrong state/,
  );
}));

async function restampAccessibility(receiptPath, mutate) {
  const directory = path.dirname(receiptPath);
  const manifestPath = path.join(directory, "feature-states.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  const state = manifest.states[0];
  const accessibilityPath = path.join(directory, state.accessibility.path);
  const accessibility = JSON.parse(await readFile(accessibilityPath, "utf8"));
  mutate(accessibility);
  const accessibilityContents = `${JSON.stringify(accessibility, null, 2)}\n`;
  await writeFile(accessibilityPath, accessibilityContents);
  state.accessibility.sha256 = createHash("sha256").update(accessibilityContents).digest("hex");
  state.accessibility.bytes = Buffer.byteLength(accessibilityContents);
  const manifestContents = `${JSON.stringify(manifest, null, 2)}\n`;
  await writeFile(manifestPath, manifestContents);
  const receipt = JSON.parse(await readFile(receiptPath, "utf8"));
  Object.assign(receipt.artifacts.find((artifact) => artifact.path === state.accessibility.path), state.accessibility);
  const index = receipt.artifacts.find((artifact) => artifact.kind === "feature-evidence");
  index.sha256 = createHash("sha256").update(manifestContents).digest("hex");
  index.bytes = Buffer.byteLength(manifestContents);
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
}

// A node the capture could not read used to be dropped silently, so the file
// claimed to be the app's accessibility tree while being a subset of it.
test("rejects a Windows accessibility snapshot that admits it is incomplete", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "windows");
  await restampAccessibility(receiptPath, (accessibility) => {
    accessibility.node_read.complete = false;
  });
  await assert.rejects(
    validateEvidence({ commit: COMMIT, evidence: [receiptPath], required: new Set(["windows"]), runId: RUN_ID }),
    /accessibility is a partial snapshot/,
  );
}));

test("rejects a Windows snapshot that counted more nodes than it carries", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "windows");
  await restampAccessibility(receiptPath, (accessibility) => {
    accessibility.node_read.read = accessibility.nodes.length + 1;
  });
  await assert.rejects(
    validateEvidence({ commit: COMMIT, evidence: [receiptPath], required: new Set(["windows"]), runId: RUN_ID }),
    /accessibility is a partial snapshot/,
  );
}));

test("rejects a Windows snapshot that never says whether the read completed", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "windows");
  await restampAccessibility(receiptPath, (accessibility) => {
    delete accessibility.node_read;
  });
  await assert.rejects(
    validateEvidence({ commit: COMMIT, evidence: [receiptPath], required: new Set(["windows"]), runId: RUN_ID }),
    /accessibility describes the wrong state/,
  );
}));

test("rejects Windows capture bounds not derived from the client", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "windows");
  const manifestPath = path.join(path.dirname(receiptPath), "feature-states.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  const state = manifest.states[0];
  const accessibilityPath = path.join(path.dirname(receiptPath), state.accessibility.path);
  const accessibility = JSON.parse(await readFile(accessibilityPath, "utf8"));
  accessibility.window.capture_bounds.kind = "outer";
  const accessibilityContents = `${JSON.stringify(accessibility, null, 2)}\n`;
  await writeFile(accessibilityPath, accessibilityContents);
  state.accessibility.sha256 = createHash("sha256").update(accessibilityContents).digest("hex");
  state.accessibility.bytes = Buffer.byteLength(accessibilityContents);
  const manifestContents = `${JSON.stringify(manifest, null, 2)}\n`;
  await writeFile(manifestPath, manifestContents);
  const receipt = JSON.parse(await readFile(receiptPath, "utf8"));
  Object.assign(receipt.artifacts.find((artifact) => artifact.path === state.accessibility.path), state.accessibility);
  const index = receipt.artifacts.find((artifact) => artifact.kind === "feature-evidence");
  index.sha256 = createHash("sha256").update(manifestContents).digest("hex");
  index.bytes = Buffer.byteLength(manifestContents);
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  await assert.rejects(
    validateEvidence({ commit: COMMIT, evidence: [receiptPath], required: new Set(["windows"]), runId: RUN_ID }),
    /accessibility describes the wrong state/,
  );
}));

test("rejects Windows accessibility evidence mapped to another feature", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "windows");
  const manifest = JSON.parse(await readFile(path.join(path.dirname(receiptPath), "feature-states.json"), "utf8"));
  const state = manifest.states[0];
  const accessibilityPath = path.join(path.dirname(receiptPath), state.accessibility.path);
  const accessibility = JSON.parse(await readFile(accessibilityPath, "utf8"));
  accessibility.feature = "devices";
  const contents = `${JSON.stringify(accessibility, null, 2)}\n`;
  await writeFile(accessibilityPath, contents);
  state.accessibility.sha256 = createHash("sha256").update(contents).digest("hex");
  state.accessibility.bytes = Buffer.byteLength(contents);
  const manifestContents = `${JSON.stringify(manifest, null, 2)}\n`;
  await writeFile(path.join(path.dirname(receiptPath), "feature-states.json"), manifestContents);
  const receipt = JSON.parse(await readFile(receiptPath, "utf8"));
  const declared = receipt.artifacts.find((artifact) => artifact.path === state.accessibility.path);
  declared.sha256 = state.accessibility.sha256;
  declared.bytes = state.accessibility.bytes;
  const index = receipt.artifacts.find((artifact) => artifact.kind === "feature-evidence");
  index.sha256 = createHash("sha256").update(manifestContents).digest("hex");
  index.bytes = Buffer.byteLength(manifestContents);
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  await assert.rejects(
    validateEvidence({ commit: COMMIT, evidence: [receiptPath], required: new Set(["windows"]), runId: RUN_ID }),
    /accessibility describes the wrong state/,
  );
}));

test("rejects a physical-device receipt in the emulator Android release slot", () => withRoot(async (root) => {
  const evidence = [
    await fixture(root, "macos"),
    await fixture(root, "android", { environment: "physical-device" }),
  ];
  await assert.rejects(
    validateEvidence({
      commit: COMMIT,
      evidence,
      required: new Set(["macos", "android"]),
      runId: RUN_ID,
    }),
    /violates the schema/,
  );
}));

test("rejects a known assertion assigned to the wrong platform", () => withRoot(async (root) => {
  const evidence = [await fixture(root, "macos", {
    assertions: [
      "installed app launched",
      "native accessibility tree is non-empty",
      "release smoke assertions passed",
    ],
  })];
  await assert.rejects(
    validateEvidence({
      commit: COMMIT,
      evidence,
      required: new Set(["macos"]),
      runId: RUN_ID,
    }),
    /violates the schema/,
  );
}));

test("rejects evidence over its measured budget", () => withRoot(async (root) => {
  const scenario = { name: "release-webview-ready", elapsed_ms: 115001, budget_ms: 115000 };
  const evidence = [await fixture(root, "android", { scenario })];
  await assert.rejects(
    validateEvidence({
      commit: COMMIT,
      evidence,
      required: new Set(["android"]),
      runId: RUN_ID,
    }),
    /exceeds its measured latency budget/,
  );
}));

for (const [description, overrides] of [
  ["scenario", { scenario: { name: "arbitrary", elapsed_ms: 10, budget_ms: 115000 } }],
  ["budget", { scenario: { name: "release-webview-ready", elapsed_ms: 10, budget_ms: 114999 } }],
  ["assertions", { assertions: ["arbitrary assertion"] }],
]) {
  test(`rejects an arbitrary Android ${description}`, () => withRoot(async (root) => {
    const evidence = [await fixture(root, "android", overrides)];
    await assert.rejects(
      validateEvidence({
        commit: COMMIT,
        evidence,
        required: new Set(["android"]),
        runId: RUN_ID,
      }),
      /violates the schema|invalid android/,
    );
  }));
}

test("rejects changed evidence bytes", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "windows");
  const receipt = JSON.parse(await readFile(receiptPath, "utf8"));
  await writeFile(path.join(path.dirname(receiptPath), receipt.artifacts[0].path), "changed\n");
  await assert.rejects(
    validateEvidence({
      commit: COMMIT,
      evidence: [receiptPath],
      required: new Set(["windows"]),
      runId: RUN_ID,
    }),
    /byte count changed|checksum changed/,
  );
}));

test("rejects a receipt for another commit", () => withRoot(async (root) => {
  const evidence = [await fixture(root, "macos")];
  await assert.rejects(
    validateEvidence({
      commit: "fedcba9876543210fedcba9876543210fedcba98",
      evidence,
      required: new Set(["macos"]),
      runId: RUN_ID,
    }),
    /belongs to another commit/,
  );
}));

test("requires the expected workflow run ID", () => withRoot(async (root) => {
  const evidence = [await fixture(root, "macos")];
  await assert.rejects(
    validateEvidence({ commit: COMMIT, evidence, required: new Set(["macos"]) }),
    /expected workflow run ID is required/,
  );
}));

test("rejects a receipt for another workflow run", () => withRoot(async (root) => {
  const evidence = [await fixture(root, "macos")];
  await assert.rejects(
    validateEvidence({
      commit: COMMIT,
      evidence,
      required: new Set(["macos"]),
      runId: "987654321",
    }),
    /belongs to another workflow run/,
  );
}));

test("rejects traversal even when it names a valid artifact", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "macos");
  const receipt = JSON.parse(await readFile(receiptPath, "utf8"));
  const outside = path.join(root, "outside.png");
  await writeFile(outside, "outside\n");
  receipt.artifacts[0] = {
    ...receipt.artifacts[0],
    path: "../outside.png",
    sha256: createHash("sha256").update("outside\n").digest("hex"),
    bytes: Buffer.byteLength("outside\n"),
  };
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  await assert.rejects(
    validateEvidence({
      commit: COMMIT,
      evidence: [receiptPath],
      required: new Set(["macos"]),
      runId: RUN_ID,
    }),
    /violates the schema/,
  );
}));

test("rejects symbolic-link artifacts", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "macos");
  const receipt = JSON.parse(await readFile(receiptPath, "utf8"));
  const target = path.join(path.dirname(receiptPath), "target");
  await mkdir(target);
  await writeFile(path.join(target, "screenshot.txt"), "macos-screenshot\n");
  await symlink(target, path.join(path.dirname(receiptPath), "linked"), "junction");
  receipt.artifacts[0].path = "linked/screenshot.txt";
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  await assert.rejects(
    validateEvidence({
      commit: COMMIT,
      evidence: [receiptPath],
      required: new Set(["macos"]),
      runId: RUN_ID,
    }),
    /uses a symbolic link/,
  );
}));

test("rejects duplicate artifact paths", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "macos");
  const receipt = JSON.parse(await readFile(receiptPath, "utf8"));
  receipt.artifacts[1] = {
    ...receipt.artifacts[1],
    path: receipt.artifacts[0].path,
    sha256: receipt.artifacts[0].sha256,
    bytes: receipt.artifacts[0].bytes,
  };
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  await assert.rejects(
    validateEvidence({
      commit: COMMIT,
      evidence: [receiptPath],
      required: new Set(["macos"]),
      runId: RUN_ID,
    }),
    /duplicate artifact paths/,
  );
}));

test("rejects hard-link aliases across artifact kinds", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "macos");
  const receipt = JSON.parse(await readFile(receiptPath, "utf8"));
  const screenshot = path.join(path.dirname(receiptPath), receipt.artifacts[0].path);
  const accessibility = path.join(path.dirname(receiptPath), receipt.artifacts[1].path);
  await unlink(screenshot);
  await link(accessibility, screenshot);
  receipt.artifacts[0].sha256 = receipt.artifacts[1].sha256;
  receipt.artifacts[0].bytes = receipt.artifacts[1].bytes;
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  await assert.rejects(
    validateEvidence({
      commit: COMMIT,
      evidence: [receiptPath],
      required: new Set(["macos"]),
      runId: RUN_ID,
    }),
    /aliases one artifact file/,
  );
}));

test("rejects duplicate artifact kinds", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "macos");
  const receipt = JSON.parse(await readFile(receiptPath, "utf8"));
  receipt.artifacts[1].kind = "screenshot";
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  await assert.rejects(
    validateEvidence({
      commit: COMMIT,
      evidence: [receiptPath],
      required: new Set(["macos"]),
      runId: RUN_ID,
    }),
    /duplicates screenshot evidence/,
  );
}));

test("rejects artifact kinds that do not belong to the platform contract", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "macos");
  const receipt = JSON.parse(await readFile(receiptPath, "utf8"));
  receipt.artifacts[1].kind = "test-log";
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  await assert.rejects(
    validateEvidence({
      commit: COMMIT,
      evidence: [receiptPath],
      required: new Set(["macos"]),
      runId: RUN_ID,
    }),
    /invalid macos artifact kind/,
  );
}));

test("the shared receipt writer emits gate-valid evidence", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "windows");
  const seeded = JSON.parse(await readFile(receiptPath, "utf8"));
  await rm(receiptPath);
  const writer = fileURLToPath(new URL("../../../scripts/release/write-native-evidence.py", import.meta.url));
  const python = process.platform === "win32" ? "python" : "python3";
  const args = [
    writer,
    "--output", receiptPath,
    "--platform", "windows",
    "--environment", "hosted-runner",
    "--os-version", "fixture-os",
    "--architecture", "fixture-arch",
    "--commit", COMMIT,
    "--run-id", RUN_ID,
    "--scenario", "windows-installed-release",
    "--elapsed-ms", "10",
    "--budget-ms", "30000",
    "--assertion", "installer integrity passed",
    "--assertion", "installed app launched",
    "--assertion", "installed sidecar launched",
    "--assertion", "named-pipe and clipboard passed",
    "--assertion", "update feed contract matched signing mode",
    "--assertion", "in-place update passed",
    "--assertion", "feature-specific UI states captured",
    "--assertion", "screenshot protection restored",
    "--assertion", "uninstall passed",
  ];
  for (const artifact of seeded.artifacts) args.push("--artifact", `${artifact.kind}=${artifact.path}`);
  await run(python, args);
  const evidence = [receiptPath];
  const receipts = await validateEvidence({
    commit: COMMIT,
    evidence,
    required: new Set(["windows"]),
    runId: RUN_ID,
  });
  assert.equal(receipts[0].platform, "windows");
}));
