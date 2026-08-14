import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { lstat, readFile, realpath } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import Ajv from "ajv";
import { verifyWindowsFeatureEvidence } from "./windows-feature-evidence.mjs";

const PLATFORM_REQUIREMENTS = {
  macos: {
    environment: "hosted-runner",
    scenario: "native-launch",
    budgetMs: 3000,
    assertions: new Set([
      "installed app launched",
      "native accessibility tree is non-empty",
    ]),
    artifacts: new Set(["screenshot", "accessibility", "measurement"]),
  },
  android: {
    environment: "physical-device",
    scenario: "release-webview-ready",
    budgetMs: 115000,
    assertions: new Set([
      "signed release app launched",
      "WebView accessibility content painted",
      "release smoke assertions passed",
    ]),
    artifacts: new Set(["screenshot", "accessibility", "measurement"]),
    optionalArtifacts: new Set(["diagnostic-log"]),
  },
  windows: {
    environment: "hosted-runner",
    scenario: "windows-installed-release",
    budgetMs: 30000,
    assertions: new Set([
      "installer integrity passed",
      "installed app launched",
      "installed sidecar launched",
      "named-pipe and clipboard passed",
      "update feed contract matched signing mode",
      "in-place update passed",
      "feature-specific UI states captured",
      "screenshot protection restored",
      "uninstall passed",
    ]),
    artifacts: new Set(["screenshot", "accessibility", "test-log", "measurement", "feature-evidence"]),
  },
};

const RUN_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._-]{0,119}$/;

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const schema = JSON.parse(
  await readFile(path.join(scriptDirectory, "native-parity-evidence.schema.json"), "utf8"),
);
const validateSchema = new Ajv({ allErrors: true, strict: true }).compile(schema);

function parseArguments(argv) {
  const evidence = [];
  const required = new Set();
  let commit;
  let runId;

  for (let index = 0; index < argv.length; index += 1) {
    const option = argv[index];
    const value = argv[index + 1];
    if (!["--evidence", "--require", "--commit", "--run-id"].includes(option) || value === undefined) {
      throw new Error("invalid command-line arguments");
    }
    index += 1;
    if (option === "--evidence") evidence.push(value);
    if (option === "--commit") commit = value;
    if (option === "--run-id") runId = value;
    if (option === "--require") {
      for (const platform of value.split(",").filter(Boolean)) required.add(platform);
    }
  }

  if (evidence.length === 0) throw new Error("at least one --evidence receipt is required");
  if (required.size === 0) throw new Error("at least one --require platform is required");
  for (const platform of required) {
    if (!(platform in PLATFORM_REQUIREMENTS)) throw new Error(`unknown required platform ${platform}`);
  }
  if (commit !== undefined && !/^[0-9a-f]{40}$/.test(commit)) {
    throw new Error("--commit must be a lowercase 40-character Git SHA");
  }
  if (!RUN_ID_PATTERN.test(runId ?? "")) throw new Error("--run-id is required and must be valid");
  return { commit, evidence, required, runId };
}

function schemaErrors() {
  return (validateSchema.errors ?? [])
    .map(({ instancePath, message }) => `${instancePath || "/"} ${message}`)
    .join("; ");
}

async function sha256(file) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(file)) hash.update(chunk);
  return hash.digest("hex");
}

async function loadReceipt(receiptPath, label) {
  let receipt;
  try {
    receipt = JSON.parse(await readFile(receiptPath, "utf8"));
  } catch {
    throw new Error(`${label} is not readable JSON`);
  }
  if (!validateSchema(receipt)) throw new Error(`${label} violates the schema: ${schemaErrors()}`);
  return receipt;
}

async function verifyArtifacts(receiptPath, receipt, label) {
  let receiptRoot;
  try {
    receiptRoot = await realpath(path.dirname(receiptPath));
  } catch {
    throw new Error(`${label} evidence directory is unavailable`);
  }
  const requirement = PLATFORM_REQUIREMENTS[receipt.platform];
  const allowedKinds = new Set([
    ...requirement.artifacts,
    ...(requirement.optionalArtifacts ?? []),
  ]);
  const kinds = new Set();
  const repeatableKinds = receipt.platform === "windows"
    ? new Set(["screenshot", "accessibility"])
    : new Set();
  const declaredPaths = new Set();
  const resolvedPaths = new Set();
  const fileIdentities = new Set();

  for (const [index, artifact] of receipt.artifacts.entries()) {
    const artifactLabel = `${label} artifact ${index + 1}`;
    if (!allowedKinds.has(artifact.kind)) {
      throw new Error(`${artifactLabel} uses an invalid ${receipt.platform} artifact kind`);
    }
    if (kinds.has(artifact.kind) && !repeatableKinds.has(artifact.kind)) {
      throw new Error(`${label} duplicates ${artifact.kind} evidence`);
    }
    kinds.add(artifact.kind);

    const candidate = path.join(receiptRoot, ...artifact.path.split("/"));
    const declaredKey = process.platform === "win32" ? candidate.toLowerCase() : candidate;
    if (declaredPaths.has(declaredKey)) throw new Error(`${label} contains duplicate artifact paths`);
    declaredPaths.add(declaredKey);

    let metadata;
    let cursor = receiptRoot;
    for (const segment of artifact.path.split("/")) {
      cursor = path.join(cursor, segment);
      try {
        metadata = await lstat(cursor);
      } catch {
        throw new Error(`${artifactLabel} is missing`);
      }
      if (metadata.isSymbolicLink()) throw new Error(`${artifactLabel} uses a symbolic link`);
    }

    let resolved;
    try {
      resolved = await realpath(candidate);
    } catch {
      throw new Error(`${artifactLabel} is missing`);
    }
    const relative = path.relative(receiptRoot, resolved);
    if (relative.startsWith("..") || path.isAbsolute(relative)) {
      throw new Error(`${artifactLabel} escapes its evidence directory`);
    }
    const resolvedKey = process.platform === "win32" ? resolved.toLowerCase() : resolved;
    if (resolvedPaths.has(resolvedKey)) throw new Error(`${label} aliases one artifact file`);
    resolvedPaths.add(resolvedKey);
    // BigInt, because `Stats.ino` is a double and an NTFS file index does not
    // fit in one. This volume reports 18577348463957901, twice
    // Number.MAX_SAFE_INTEGER, so consecutive MFT records round to the same
    // value and two distinct evidence files read as one hard link.
    const exact = await lstat(resolved, { bigint: true });
    const identity = `${exact.dev}:${exact.ino}`;
    if ((exact.dev !== 0n || exact.ino !== 0n) && fileIdentities.has(identity)) {
      throw new Error(`${label} aliases one artifact file`);
    }
    fileIdentities.add(identity);
    if (!metadata.isFile()) throw new Error(`${artifactLabel} is not a regular file`);

    let digest;
    try {
      digest = await sha256(resolved);
    } catch {
      throw new Error(`${artifactLabel} cannot be read`);
    }
    if (metadata.size !== artifact.bytes) throw new Error(`${artifactLabel} byte count changed`);
    if (digest !== artifact.sha256) throw new Error(`${artifactLabel} checksum changed`);
  }

  const needed = requirement.artifacts;
  const missing = [...needed].filter((kind) => !kinds.has(kind));
  if (missing.length > 0) throw new Error(`${label} lacks ${missing.join(", ")} evidence`);
}

export async function validateEvidence({ commit, evidence, required, runId }) {
  if (!RUN_ID_PATTERN.test(runId ?? "")) throw new Error("expected workflow run ID is required");
  const receipts = new Map();
  let observedCommit = commit;

  for (const [index, receiptPath] of evidence.entries()) {
    const label = `receipt ${index + 1}`;
    const receipt = await loadReceipt(receiptPath, label);
    if (receipts.has(receipt.platform)) throw new Error(`duplicate ${receipt.platform} receipt`);
    const requirement = PLATFORM_REQUIREMENTS[receipt.platform];
    if (receipt.environment !== requirement.environment) {
      throw new Error(`${label} uses an invalid ${receipt.platform} environment`);
    }
    if (receipt.scenario.name !== requirement.scenario) {
      throw new Error(`${label} uses an invalid ${receipt.platform} scenario`);
    }
    if (receipt.scenario.budget_ms !== requirement.budgetMs) {
      throw new Error(`${label} uses an invalid ${receipt.platform} latency budget`);
    }
    if (receipt.scenario.elapsed_ms > receipt.scenario.budget_ms) {
      throw new Error(`${label} exceeds its measured latency budget`);
    }
    const assertions = new Set(receipt.assertions);
    if (
      assertions.size !== requirement.assertions.size
      || [...requirement.assertions].some((assertion) => !assertions.has(assertion))
    ) {
      throw new Error(`${label} uses invalid ${receipt.platform} assertions`);
    }
    observedCommit ??= receipt.source.commit;
    if (receipt.source.commit !== observedCommit) throw new Error(`${label} belongs to another commit`);
    if (receipt.source.run_id !== runId) throw new Error(`${label} belongs to another workflow run`);
    await verifyArtifacts(receiptPath, receipt, label);
    if (receipt.platform === "windows") {
      await verifyWindowsFeatureEvidence(receiptPath, receipt, label);
    }
    receipts.set(receipt.platform, receipt);
  }

  const actual = new Set(receipts.keys());
  const missing = [...required].filter((platform) => !actual.has(platform));
  const unexpected = [...actual].filter((platform) => !required.has(platform));
  if (missing.length > 0) throw new Error(`missing required evidence for ${missing.join(", ")}`);
  if (unexpected.length > 0) throw new Error(`unexpected evidence for ${unexpected.join(", ")}`);
  return [...receipts.values()];
}

export async function main(argv = process.argv.slice(2)) {
  const options = parseArguments(argv);
  const receipts = await validateEvidence(options);
  for (const receipt of receipts.sort((left, right) => left.platform.localeCompare(right.platform))) {
    console.log(
      `native-parity: ${receipt.platform} ${receipt.environment}, ${receipt.scenario.name} ` +
        `${receipt.scenario.elapsed_ms}/${receipt.scenario.budget_ms} ms`,
    );
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    console.error(`native-parity: ${error.message}`);
    process.exitCode = 1;
  });
}
