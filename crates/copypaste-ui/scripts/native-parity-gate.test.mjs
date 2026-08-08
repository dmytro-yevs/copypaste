import { createHash } from "node:crypto";
import { execFile } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

import { validateEvidence } from "./native-parity-gate.mjs";

const COMMIT = "0123456789abcdef0123456789abcdef01234567";
const run = promisify(execFile);

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
    environment: platform === "android" ? "physical-device" : "hosted-runner",
    os_version: "fixture-os",
    architecture: "fixture-arch",
    source: { commit: COMMIT, run_id: "fixture-run" },
    scenario: { name: "fixture", elapsed_ms: 10, budget_ms: 20 },
    assertions: ["fixture assertion passed"],
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
  });
  assert.equal(receipts.length, 3);
}));

test("fails closed when a required platform is absent", () => withRoot(async (root) => {
  const evidence = [await fixture(root, "macos")];
  await assert.rejects(
    validateEvidence({ commit: COMMIT, evidence, required: new Set(["macos", "android"]) }),
    /missing required evidence for android/,
  );
}));

test("rejects an emulator receipt pretending to be macOS", () => withRoot(async (root) => {
  const evidence = [await fixture(root, "macos", { environment: "emulator" })];
  await assert.rejects(
    validateEvidence({ commit: COMMIT, evidence, required: new Set(["macos"]) }),
    /invalid macos environment/,
  );
}));

test("rejects evidence over its measured budget", () => withRoot(async (root) => {
  const scenario = { name: "slow", elapsed_ms: 21, budget_ms: 20 };
  const evidence = [await fixture(root, "android", { scenario })];
  await assert.rejects(
    validateEvidence({ commit: COMMIT, evidence, required: new Set(["android"]) }),
    /exceeds its measured latency budget/,
  );
}));

test("rejects changed evidence bytes", () => withRoot(async (root) => {
  const receiptPath = await fixture(root, "windows");
  const receipt = JSON.parse(await readFile(receiptPath, "utf8"));
  await writeFile(path.join(path.dirname(receiptPath), receipt.artifacts[0].path), "changed\n");
  await assert.rejects(
    validateEvidence({ commit: COMMIT, evidence: [receiptPath], required: new Set(["windows"]) }),
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
    }),
    /belongs to another commit/,
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
    "--run-id", "fixture-run",
    "--scenario", "windows-native-contracts",
    "--elapsed-ms", "10",
    "--budget-ms", "20",
    "--assertion", "native contract passed",
    "--artifact", "test-log=native-tests.log",
    "--artifact", "measurement=latency.json",
  ]);
  const evidence = [path.join(output, "native-evidence.json")];
  const receipts = await validateEvidence({
    commit: COMMIT,
    evidence,
    required: new Set(["windows"]),
  });
  assert.equal(receipts[0].platform, "windows");
}));
