#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { createRequire } from "node:module";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const repo = join(dirname(fileURLToPath(import.meta.url)), "../..");
const uiPackagePath = join(repo, "crates/copypaste-ui/package.json");
const requireFromUi = createRequire(pathToFileURL(uiPackagePath));

function jsoncParser() {
  return requireFromUi("jsonc-parser");
}

function semver() {
  return requireFromUi("semver");
}

export function versionCodeFor(input) {
  const version = semver().parse(input, { loose: false });
  if (!version || version.raw !== input || version.build.length !== 0) {
    throw new Error(`unsupported product version '${input}'`);
  }
  if (version.major > 20 || version.minor > 99 || version.patch > 99) {
    throw new Error(`version '${input}' exceeds the Android versionCode allocation`);
  }

  let stage = 9999;
  if (version.prerelease.length !== 0) {
    if (version.prerelease.length !== 2 || typeof version.prerelease[1] !== "number") {
      throw new Error(`prerelease '${input}' must be alpha.N, beta.N, or rc.N`);
    }
    const [channel, sequence] = version.prerelease;
    const bases = { alpha: 0, beta: 3000, rc: 6000 };
    if (!(channel in bases) || sequence > 2999) {
      throw new Error(`prerelease '${input}' exceeds the Android channel allocation`);
    }
    stage = bases[channel] + sequence;
  }

  return ((version.major * 10_000 + version.minor * 100 + version.patch) * 10_000) + stage;
}

export function previousFixtureVersion(input) {
  const version = semver().parse(input, { loose: false });
  if (!version) throw new Error(`invalid product version '${input}'`);
  if (version.prerelease.length === 2 && typeof version.prerelease[1] === "number") {
    const [channel, sequence] = version.prerelease;
    if (sequence > 0) return `${version.major}.${version.minor}.${version.patch}-${channel}.${sequence - 1}`;
    if (channel === "rc") return `${version.major}.${version.minor}.${version.patch}-beta.2999`;
    if (channel === "beta") return `${version.major}.${version.minor}.${version.patch}-alpha.2999`;
    if (channel === "alpha") {
      if (version.patch > 0) return `${version.major}.${version.minor}.${version.patch - 1}`;
      if (version.minor > 0) return `${version.major}.${version.minor - 1}.99`;
      if (version.major > 0) return `${version.major - 1}.99.99`;
    }
  }
  if (version.prerelease.length === 0) {
    return `${version.major}.${version.minor}.${version.patch}-rc.2999`;
  }
  throw new Error(`cannot derive an upgrade fixture before '${input}'`);
}

function repositoryProduct() {
  const raw = execFileSync("cargo", [
    "metadata", "--no-deps", "--format-version", "1",
    "--manifest-path", join(repo, "Cargo.toml"),
  ], { encoding: "utf8" });
  const metadata = JSON.parse(raw);
  const ui = metadata.packages.find((item) => item.name === "copypaste-ui");
  const product = metadata.metadata?.copypaste;
  if (!ui) throw new Error("cargo metadata did not contain copypaste-ui");
  if (!product || typeof product !== "object") {
    throw new Error("Cargo.toml is missing workspace.metadata.copypaste");
  }
  const releaseApplicationId = product["android-release-application-id"];
  const debugSuffix = product["android-debug-application-id-suffix"];
  const releaseCertificateSha256 = product["android-release-certificate-sha256"];
  if (typeof releaseApplicationId !== "string" || !/^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$/.test(releaseApplicationId)) {
    throw new Error("Cargo.toml contains an invalid Android release application id");
  }
  if (typeof debugSuffix !== "string" || !/^\.[a-z][a-z0-9_]*$/.test(debugSuffix)) {
    throw new Error("Cargo.toml contains an invalid Android debug application id suffix");
  }
  if (typeof releaseCertificateSha256 !== "string" || !/^[0-9a-f]{64}$/.test(releaseCertificateSha256)) {
    throw new Error("Cargo.toml must contain one lowercase Android release certificate SHA-256 fingerprint");
  }
  return {
    versionName: ui.version,
    releaseApplicationId,
    debugApplicationIdSuffix: debugSuffix,
    debugApplicationId: `${releaseApplicationId}${debugSuffix}`,
    releaseCertificateSha256,
  };
}

export function resolveIdentity() {
  const product = repositoryProduct();
  return {
    releaseApplicationId: product.releaseApplicationId,
    debugApplicationId: product.debugApplicationId,
  };
}

function updateAndroidConfig(source, product) {
  const { applyEdits, modify } = jsoncParser();
  let updated = source;
  const fields = [
    [["identifier"], product.releaseApplicationId],
    [["bundle", "android", "versionCode"], versionCodeFor(product.versionName)],
    [["bundle", "android", "debugApplicationIdSuffix"], product.debugApplicationIdSuffix],
  ];
  for (const [path, value] of fields) {
    const edits = modify(updated, path, value, {
      formattingOptions: { insertSpaces: true, tabSize: 2, eol: "\n" },
    });
    updated = applyEdits(updated, edits);
  }
  return updated;
}

export function syncAndroidConfig(path, product) {
  const source = existsSync(path) ? readFileSync(path, "utf8") : "{}\n";
  const updated = updateAndroidConfig(source, product);
  if (source === updated) return false;
  writeFileSync(path, updated);
  return true;
}

export function writeVersionOverlay(path, versionName) {
  const overlay = {
    version: versionName,
    bundle: { android: { versionCode: versionCodeFor(versionName) } },
  };
  writeFileSync(path, `${JSON.stringify(overlay, null, 2)}\n`);
}

function validateVersionOverride(versionOverride) {
  if (versionOverride && process.env.COPYPASTE_ANDROID_UPGRADE_FIXTURE !== "1") {
    throw new Error("a version override is allowed only for the CI upgrade fixture");
  }
}

export function resolveMetadata(versionOverride) {
  const product = repositoryProduct();
  const tauri = JSON.parse(readFileSync(join(repo, "crates/copypaste-ui/src-tauri/tauri.conf.json"), "utf8"));
  const androidPath = join(repo, "crates/copypaste-ui/src-tauri/tauri.android.conf.json");
  const uiPackage = JSON.parse(readFileSync(uiPackagePath, "utf8"));
  const lock = JSON.parse(readFileSync(join(repo, "crates/copypaste-ui/package-lock.json"), "utf8"));
  validateVersionOverride(versionOverride);
  const versionName = versionOverride ?? product.versionName;
  if (uiPackage.version !== product.versionName) {
    throw new Error(`product version mismatch: Cargo.toml=${product.versionName}, package.json=${uiPackage.version}`);
  }
  if (lock.version !== product.versionName || lock.packages?.[""]?.version !== product.versionName) {
    throw new Error(`product version mismatch: Cargo.toml=${product.versionName}, package-lock.json is stale`);
  }
  if (tauri.version !== "../package.json") {
    throw new Error(`tauri.conf.json must resolve version through ../package.json`);
  }
  const android = jsoncParser().parse(readFileSync(androidPath, "utf8"));
  if (android?.identifier !== product.releaseApplicationId
      || android?.bundle?.android?.versionCode !== versionCodeFor(product.versionName)
      || android?.bundle?.android?.debugApplicationIdSuffix !== product.debugApplicationIdSuffix) {
    throw new Error("tauri.android.conf.json is stale; run --sync");
  }

  return {
    ...product,
    versionName,
    versionCode: versionCodeFor(versionName),
  };
}

function syncDerivatives() {
  const product = repositoryProduct();
  const versionName = product.versionName;
  const packagePath = uiPackagePath;
  const lockPath = join(repo, "crates/copypaste-ui/package-lock.json");
  const androidPath = join(repo, "crates/copypaste-ui/src-tauri/tauri.android.conf.json");
  const uiPackage = JSON.parse(readFileSync(packagePath, "utf8"));
  const lock = JSON.parse(readFileSync(lockPath, "utf8"));
  if (uiPackage.version !== versionName || "copypaste" in uiPackage) {
    uiPackage.version = versionName;
    delete uiPackage.copypaste;
    writeFileSync(packagePath, `${JSON.stringify(uiPackage, null, 2)}\n`);
  }
  if (lock.version !== versionName || lock.packages[""].version !== versionName) {
    lock.version = versionName;
    lock.packages[""].version = versionName;
    writeFileSync(lockPath, `${JSON.stringify(lock, null, 2)}\n`);
  }
  syncAndroidConfig(androidPath, product);
}

function main() {
  const args = process.argv.slice(2);
  let format = "json";
  let field;
  let versionOverride;
  let overlayPath;
  let sync = false;
  for (let index = 0; index < args.length; index += 1) {
    if (args[index] === "--format") format = args[++index];
    else if (args[index] === "--field") field = args[++index];
    else if (args[index] === "--version") versionOverride = args[++index];
    else if (args[index] === "--write-overlay") overlayPath = args[++index];
    else if (args[index] === "--sync") sync = true;
    else if (args[index] === "--check") format = "json";
    else if (args[index] === "--previous-fixture") {
      process.stdout.write(`${previousFixtureVersion(repositoryProduct().versionName)}\n`);
      return;
    } else throw new Error(`unknown argument '${args[index]}'`);
  }

  if (overlayPath) {
    validateVersionOverride(versionOverride);
    if (!versionOverride) throw new Error("--write-overlay requires --version");
    writeVersionOverlay(overlayPath, versionOverride);
    return;
  }
  if (sync) {
    if (versionOverride) throw new Error("--sync does not accept --version; use --write-overlay");
    syncDerivatives();
    return;
  }
  if (["releaseApplicationId", "debugApplicationId"].includes(field)) {
    process.stdout.write(`${resolveIdentity()[field]}\n`);
    return;
  }
  if (field === "releaseCertificateSha256") {
    process.stdout.write(`${repositoryProduct().releaseCertificateSha256}\n`);
    return;
  }
  const metadata = resolveMetadata(versionOverride);
  if (field) {
    if (!(field in metadata)) throw new Error(`unknown metadata field '${field}'`);
    process.stdout.write(`${metadata[field]}\n`);
  } else if (format === "properties") {
    for (const [key, value] of Object.entries(metadata)) process.stdout.write(`${key}=${value}\n`);
  } else if (format === "json") {
    process.stdout.write(`${JSON.stringify(metadata)}\n`);
  } else throw new Error(`unknown output format '${format}'`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try { main(); } catch (error) {
    console.error(`android metadata: ${error.message}`);
    process.exitCode = 1;
  }
}
