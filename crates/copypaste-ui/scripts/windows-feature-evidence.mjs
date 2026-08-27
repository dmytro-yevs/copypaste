import { createHash } from "node:crypto";
import { lstat, readFile, realpath } from "node:fs/promises";
import path from "node:path";

const EXPECTED = new Map([
  ["history/populated", { feature: "history", state: "populated", name: "Clipboard history", directory: "history", direct: true }],
  ["capture/service-capture-status", { feature: "capture", state: "service-capture-status", name: "Background capture", directory: "capture", direct: true }],
  ["capture/copy-feedback-setting", { feature: "capture", state: "copy-feedback-setting", name: "Copy feedback sound", directory: "capture/copy-feedback-setting", direct: false }],
  ["devices/desktop-pairing-entry", {
    feature: "devices",
    state: "desktop-pairing-entry",
    name: "Enter pairing code",
    requiredNames: ["Show pairing code", "Enter pairing code"],
    directory: "devices",
    direct: true,
  }],
  ["settings-and-service/appearance", { feature: "settings-and-service", state: "appearance", name: "Mode", directory: "settings-and-service", direct: true }],
  ["cloud-account/unconfigured", { feature: "cloud-account", state: "unconfigured", name: "Cloud server configuration", directory: "cloud-account", direct: true }],
]);

const UPDATER_STATES = new Map([
  ["updater-configured", { feature: "settings-and-service", state: "updater-configured", name: "Check for updates", directory: "settings-and-service/updater-configured", direct: false }],
  ["updater-unconfigured", { feature: "settings-and-service", state: "updater-unconfigured", name: "Updates aren't configured in this build.", directory: "settings-and-service/updater-unconfigured", direct: false }],
]);

const RECORD_KEYS = ["bytes", "path", "sha256"];
const STATE_KEYS = ["accessibility", "expected_name", "feature", "screenshot", "state"];
const ACCESSIBILITY_KEYS = ["expected_name", "feature", "node_read", "nodes", "schema_version", "state", "window"];
const NODE_READ_KEYS = ["complete", "read", "retried"];
const WINDOW_KEYS = ["capture_allowed", "capture_bounds", "display_affinity", "foreground", "handle", "minimized", "visible"];
const BOUNDS_KEYS = ["height", "kind", "width", "x", "y"];

function parseJson(bytes) {
  return JSON.parse(bytes.toString("utf8").replace(/^\uFEFF/, ""));
}

function exactKeys(value, expected) {
  return value !== null
    && typeof value === "object"
    && !Array.isArray(value)
    && JSON.stringify(Object.keys(value).sort()) === JSON.stringify([...expected].sort());
}

async function verifyFile(root, record, expectedPath, label) {
  if (!exactKeys(record, RECORD_KEYS) || record.path !== expectedPath) {
    throw new Error(`${label} has an invalid file record`);
  }
  if (!Number.isInteger(record.bytes) || record.bytes < 1 || !/^[0-9a-f]{64}$/.test(record.sha256)) {
    throw new Error(`${label} has invalid file identity`);
  }
  const candidate = path.join(root, ...record.path.split("/"));
  const metadata = await lstat(candidate).catch(() => null);
  if (!metadata?.isFile() || metadata.isSymbolicLink()) throw new Error(`${label} is missing or unsafe`);
  const resolved = await realpath(candidate);
  const relative = path.relative(root, resolved);
  if (relative.startsWith("..") || path.isAbsolute(relative)) throw new Error(`${label} escapes its receipt`);
  const bytes = await readFile(resolved);
  const digest = createHash("sha256").update(bytes).digest("hex");
  if (bytes.length !== record.bytes || digest !== record.sha256) throw new Error(`${label} changed after capture`);
  return bytes;
}

export async function verifyWindowsFeatureEvidence(receiptPath, receipt, label, updaterState) {
  const expected = new Map(EXPECTED);
  const expectedUpdater = UPDATER_STATES.get(updaterState);
  if (!expectedUpdater) throw new Error(`${label} has an invalid expected Windows updater state`);
  expected.set(`settings-and-service/${updaterState}`, expectedUpdater);
  const index = receipt.artifacts.find((artifact) => artifact.kind === "feature-evidence");
  if (!index) throw new Error(`${label} lacks Windows feature evidence`);
  let manifest;
  try {
    manifest = parseJson(await readFile(path.join(path.dirname(receiptPath), index.path)));
  } catch {
    throw new Error(`${label} feature evidence is not readable JSON`);
  }
  if (!exactKeys(manifest, ["schema_version", "states"]) || manifest.schema_version !== 1 || !Array.isArray(manifest.states)) {
    throw new Error(`${label} feature evidence has an invalid envelope`);
  }
  if (manifest.states.length !== expected.size) throw new Error(`${label} must capture the exact Windows feature state set`);

  const root = await realpath(path.dirname(receiptPath));
  const observed = new Set();
  const screenshotIdentities = new Set();
  for (const state of manifest.states) {
    const identity = `${state?.feature}/${state?.state}`;
    if (!exactKeys(state, STATE_KEYS) || observed.has(identity)) {
      throw new Error(`${label} contains an invalid or duplicate Windows feature state`);
    }
    observed.add(identity);
    const expectedState = expected.get(identity);
    if (!expectedState || state.expected_name !== expectedState.name) {
      throw new Error(`${label} contains an unknown or wrong Windows feature state ${identity}`);
    }
    const prefix = `${expectedState.directory}/`;
    for (const [kind, record] of [["screenshot", state.screenshot], ["accessibility", state.accessibility]]) {
      const declared = receipt.artifacts.find((artifact) => artifact.kind === kind && artifact.path === record.path);
      if (expectedState.direct && (!declared || declared.sha256 !== record.sha256 || declared.bytes !== record.bytes)) {
        throw new Error(`${label} does not directly register ${state.feature} ${kind} evidence`);
      }
    }
    if (screenshotIdentities.has(state.screenshot.sha256)) {
      throw new Error(`${label} reuses a screenshot identity across Windows feature states`);
    }
    screenshotIdentities.add(state.screenshot.sha256);
    await verifyFile(root, state.screenshot, `${prefix}screenshot.png`, `${label} ${state.feature} screenshot`);
    const accessibilityBytes = await verifyFile(
      root,
      state.accessibility,
      `${prefix}accessibility.json`,
      `${label} ${state.feature} accessibility`,
    );
    let accessibility;
    try {
      accessibility = parseJson(accessibilityBytes);
    } catch {
      throw new Error(`${label} ${state.feature} accessibility is not JSON`);
    }
    if (
      !exactKeys(accessibility, ACCESSIBILITY_KEYS)
      || accessibility.schema_version !== 2
      || accessibility.feature !== state.feature
      || accessibility.state !== state.state
      || accessibility.expected_name !== state.expected_name
      || !exactKeys(accessibility.window, WINDOW_KEYS)
      || !Number.isInteger(accessibility.window.handle)
      || accessibility.window.handle < 1
      || accessibility.window.foreground !== true
      || accessibility.window.visible !== true
      || accessibility.window.minimized !== false
      || accessibility.window.capture_allowed !== true
      || accessibility.window.display_affinity !== 0
      || !exactKeys(accessibility.window.capture_bounds, BOUNDS_KEYS)
      || accessibility.window.capture_bounds.kind !== "client"
      || !Number.isInteger(accessibility.window.capture_bounds.x)
      || !Number.isInteger(accessibility.window.capture_bounds.y)
      || !Number.isInteger(accessibility.window.capture_bounds.width)
      || !Number.isInteger(accessibility.window.capture_bounds.height)
      || accessibility.window.capture_bounds.width < 1
      || accessibility.window.capture_bounds.height < 1
      || accessibility.window.capture_bounds.width > 16384
      || accessibility.window.capture_bounds.height > 16384
    ) {
      throw new Error(`${label} ${state.feature} accessibility describes the wrong state`);
    }
    // A node the capture could not read used to be dropped, so a subset of the
    // tree arrived here indistinguishable from the whole of it. The count is
    // checked against the array as well as the flag: a receipt that claims a
    // complete read and carries fewer nodes than it counted is not evidence.
    if (
      !exactKeys(accessibility.node_read, NODE_READ_KEYS)
      || accessibility.node_read.complete !== true
      || !Array.isArray(accessibility.node_read.retried)
      || !Array.isArray(accessibility.nodes)
      || !Number.isInteger(accessibility.node_read.read)
      || accessibility.node_read.read < 1
      || accessibility.node_read.read !== accessibility.nodes.length
    ) {
      throw new Error(`${label} ${state.feature} accessibility is a partial snapshot`);
    }
    const marker = Array.isArray(accessibility.nodes)
      ? accessibility.nodes.find((node) => (
        node?.name === expectedState.name
        && node.enabled === true
        && node.offscreen === false
        && Number.isFinite(node.bounds?.width)
        && Number.isFinite(node.bounds?.height)
        && node.bounds.width > 0
        && node.bounds.height > 0
      ))
      : null;
    if (!marker) {
      throw new Error(`${label} ${state.feature} accessibility does not prove its expected UI state`);
    }
    for (const requiredName of expectedState.requiredNames ?? []) {
      if (!accessibility.nodes.some((node) => (
        node?.name === requiredName
        && node.enabled === true
        && node.offscreen === false
        && Number.isFinite(node.bounds?.width)
        && Number.isFinite(node.bounds?.height)
        && node.bounds.width > 0
        && node.bounds.height > 0
      ))) {
        throw new Error(`${label} ${state.feature} accessibility lacks ${requiredName}`);
      }
    }
  }
  if ([...expected.keys()].some((identity) => !observed.has(identity))) {
    throw new Error(`${label} omits a Windows feature state`);
  }
}
