import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { readFile, realpath, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import Ajv from "ajv";

const PLATFORM_REQUIREMENTS = {
  macos: {
    environments: new Set(["hosted-runner", "physical-device"]),
    artifacts: new Set(["screenshot", "accessibility", "measurement"]),
  },
  android: {
    environments: new Set(["emulator", "physical-device"]),
    artifacts: new Set(["screenshot", "accessibility", "measurement"]),
  },
  windows: {
    environments: new Set(["hosted-runner", "physical-device"]),
    artifacts: new Set(["test-log", "measurement"]),
  },
};

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const schema = JSON.parse(
  await readFile(path.join(scriptDirectory, "native-parity-evidence.schema.json"), "utf8"),
);
const validateSchema = new Ajv({ allErrors: true, strict: true }).compile(schema);

function parseArguments(argv) {
  const evidence = [];
  const required = new Set();
  let commit;

  for (let index = 0; index < argv.length; index += 1) {
    const option = argv[index];
    const value = argv[index + 1];
    if (!["--evidence", "--require", "--commit"].includes(option) || value === undefined) {
      throw new Error("invalid command-line arguments");
    }
    index += 1;
    if (option === "--evidence") evidence.push(value);
    if (option === "--commit") commit = value;
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
  return { commit, evidence, required };
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
  const kinds = new Set();

  for (const [index, artifact] of receipt.artifacts.entries()) {
    const artifactLabel = `${label} artifact ${index + 1}`;
    const candidate = path.resolve(receiptRoot, artifact.path);
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
    let metadata;
    let digest;
    try {
      metadata = await stat(resolved);
      digest = await sha256(resolved);
    } catch {
      throw new Error(`${artifactLabel} cannot be read`);
    }
    if (!metadata.isFile()) throw new Error(`${artifactLabel} is not a regular file`);
    if (metadata.size !== artifact.bytes) throw new Error(`${artifactLabel} byte count changed`);
    if (digest !== artifact.sha256) throw new Error(`${artifactLabel} checksum changed`);
    kinds.add(artifact.kind);
  }

  const needed = PLATFORM_REQUIREMENTS[receipt.platform].artifacts;
  const missing = [...needed].filter((kind) => !kinds.has(kind));
  if (missing.length > 0) throw new Error(`${label} lacks ${missing.join(", ")} evidence`);
}

export async function validateEvidence({ commit, evidence, required }) {
  const receipts = new Map();
  let observedCommit = commit;
  let observedRunId;

  for (const [index, receiptPath] of evidence.entries()) {
    const label = `receipt ${index + 1}`;
    const receipt = await loadReceipt(receiptPath, label);
    if (receipts.has(receipt.platform)) throw new Error(`duplicate ${receipt.platform} receipt`);
    const allowed = PLATFORM_REQUIREMENTS[receipt.platform].environments;
    if (!allowed.has(receipt.environment)) {
      throw new Error(`${label} uses an invalid ${receipt.platform} environment`);
    }
    if (receipt.scenario.elapsed_ms > receipt.scenario.budget_ms) {
      throw new Error(`${label} exceeds its measured latency budget`);
    }
    observedCommit ??= receipt.source.commit;
    if (receipt.source.commit !== observedCommit) throw new Error(`${label} belongs to another commit`);
    observedRunId ??= receipt.source.run_id;
    if (receipt.source.run_id !== observedRunId) throw new Error(`${label} belongs to another workflow run`);
    await verifyArtifacts(receiptPath, receipt, label);
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
