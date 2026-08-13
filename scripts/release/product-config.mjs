import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const repo = join(dirname(fileURLToPath(import.meta.url)), "../..");
const requireFromUi = createRequire(pathToFileURL(join(repo, "crates/copypaste-ui/package.json")));
const androidNamespace = "http://schemas.android.com/apk/res/android";
const deepLinkPermission = "deep-link:default";

function jsoncParser() {
  return requireFromUi("jsonc-parser");
}

function xmlDom() {
  return requireFromUi("@xmldom/xmldom");
}

function validateDeepLinkScheme(scheme) {
  if (typeof scheme !== "string" || !/^[a-z][a-z0-9+.-]*$/.test(scheme)) {
    throw new Error(`invalid deep-link scheme '${scheme}'`);
  }
}

function updateJson(source, fields) {
  const { applyEdits, modify, parse } = jsoncParser();
  let updated = source;
  for (const [path, value] of fields) {
    const current = path.reduce((parent, key) => parent?.[key], parse(updated));
    if (JSON.stringify(current) === JSON.stringify(value)) continue;
    updated = applyEdits(updated, modify(updated, path, value, {
      formattingOptions: { insertSpaces: true, tabSize: 2, eol: "\n" },
    }));
  }
  return updated;
}

function androidAttribute(element, name) {
  return element.getAttributeNS(androidNamespace, name);
}

function childElements(element, name) {
  return Array.from(element.childNodes)
    .filter((node) => node.nodeType === 1 && node.tagName === name);
}

function deepLinkDataElement(document) {
  const matches = [];
  for (const activity of Array.from(document.getElementsByTagName("activity"))) {
    if (androidAttribute(activity, "name") !== ".MainActivity") continue;
    for (const filter of childElements(activity, "intent-filter")) {
      const isView = childElements(filter, "action")
        .some((action) => androidAttribute(action, "name") === "android.intent.action.VIEW");
      if (!isView) continue;
      matches.push(...childElements(filter, "data")
        .filter((data) => androidAttribute(data, "host") === "pair"));
    }
  }
  if (matches.length !== 1) {
    throw new Error(`AndroidManifest.xml must contain one MainActivity VIEW data element for host 'pair'; found ${matches.length}`);
  }
  return matches[0];
}

export function updateTauriDeepLinkConfig(source, scheme) {
  validateDeepLinkScheme(scheme);
  return updateJson(source, [
    [["plugins", "deep-link", "desktop", "schemes"], [scheme]],
    [["plugins", "deep-link", "mobile", 0, "scheme"], [scheme]],
  ]);
}

export function updateCapabilityDeepLinkConfig(source, scheme) {
  validateDeepLinkScheme(scheme);
  const { parse } = jsoncParser();
  const permissions = parse(source)?.permissions;
  if (!Array.isArray(permissions)) {
    throw new Error("capabilities/default.json must contain a permissions array");
  }
  const firstDeepLink = permissions.findIndex((permission) =>
    typeof permission === "string" && permission.startsWith("deep-link:"));
  const retained = permissions.filter((permission) =>
    typeof permission !== "string" || !permission.startsWith("deep-link:"));
  retained.splice(firstDeepLink < 0 ? retained.length : firstDeepLink, 0, deepLinkPermission);
  const permissionPath = ["permissions"];
  return updateJson(source, [[permissionPath, retained]]);
}

export function updateAndroidDeepLinkManifest(source, scheme) {
  validateDeepLinkScheme(scheme);
  const { DOMParser } = xmlDom();
  const errors = [];
  const document = new DOMParser({
    onError: (level, message) => errors.push(`${level}: ${message}`),
  }).parseFromString(source, "application/xml");
  if (errors.length !== 0) {
    throw new Error(`AndroidManifest.xml is not valid XML: ${errors.join("; ")}`);
  }
  const current = androidAttribute(deepLinkDataElement(document), "scheme");
  if (current === scheme) return source;
  const attribute = `android:scheme="${current}"`;
  if (source.split(attribute).length !== 2) {
    throw new Error("AndroidManifest.xml deep-link scheme must use one double-quoted android:scheme attribute");
  }
  return source.replace(attribute, `android:scheme="${scheme}"`);
}

export function projectDeepLinkConfigs(configs, scheme) {
  return {
    tauri: updateTauriDeepLinkConfig(configs.tauri, scheme),
    androidManifest: updateAndroidDeepLinkManifest(configs.androidManifest, scheme),
    capability: updateCapabilityDeepLinkConfig(configs.capability, scheme),
  };
}

export function staleDeepLinkConfigs(configs, scheme) {
  const projected = projectDeepLinkConfigs(configs, scheme);
  return Object.keys(projected).filter((name) => projected[name] !== configs[name]);
}
