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

const RECEIPT_VALUES = {
  macos: {
    environment: "hosted-runner",
    scenario: { name: "native-launch", elapsed_ms: 10, budget_ms: 3000 },
    assertions: ["installed app launched", "native accessibility tree is non-empty"],
  },
  android: {
    environment: "physical-device",
    scenario: { name: "release-webview-ready", elapsed_ms: 10, budget_ms: 115000 },
    assertions: [
      "signed release app launched",
      "WebView accessibility content painted",
      "release smoke assertions passed",
    ],
  },
  windows: {
    environment: "hosted-runner",
    scenario: { name: "windows-native-contracts", elapsed_ms: 10, budget_ms: 900000 },
    assertions: ["named-pipe contract passed", "DPAPI round-trip passed"],
  },
};

async function fixture(root, platform, overrides = {}) {
  const directory = path.join(root, platform);
  const artifacts = [];
  const kinds = platform === "windows"
    ? ["test-log", "measurement"]
    : ["screenshot", "accessibility", "measurement"];
  await mkdir(directory, { recursive: true });

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
    ...overrides,
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

test("accepts exact macOS, Android, and requested Windows evidence", () => withRoot(async (root) => {
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

test("rejects an emulator receipt in the physical Android release slot", () => withRoot(async (root) => {
  const evidence = [
    await fixture(root, "macos"),
    await fixture(root, "android", { environment: "emulator" }),
  ];
  await assert.rejects(
    validateEvidence({
      commit: COMMIT,
      evidence,
      required: new Set(["macos", "android"]),
      runId: RUN_ID,
    }),
    /invalid android environment/,
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
  const output = path.join(root, "windows");
  await mkdir(output, { recursive: true });
  await writeFile(path.join(output, "native-tests.log"), "native tests passed\n");
  await writeFile(path.join(output, "latency.json"), '{"elapsed_ms":10}\n');
  const writer = fileURLToPath(new URL("../../../scripts/release/write-native-evidence.py", import.meta.url));
  const python = process.platform === "win32" ? "python" : "python3";
  await run(python, [
    writer,
    "--output", path.join(output, "native-evidence.json"),
    "--platform", "windows",
    "--environment", "hosted-runner",
    "--os-version", "fixture-os",
    "--architecture", "fixture-arch",
    "--commit", COMMIT,
    "--run-id", RUN_ID,
    "--scenario", "windows-native-contracts",
    "--elapsed-ms", "10",
    "--budget-ms", "900000",
    "--assertion", "named-pipe contract passed",
    "--assertion", "DPAPI round-trip passed",
    "--artifact", "test-log=native-tests.log",
    "--artifact", "measurement=latency.json",
  ]);
  const evidence = [path.join(output, "native-evidence.json")];
  const receipts = await validateEvidence({
    commit: COMMIT,
    evidence,
    required: new Set(["windows"]),
    runId: RUN_ID,
  });
  assert.equal(receipts[0].platform, "windows");
}));
