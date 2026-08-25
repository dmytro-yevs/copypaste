import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const repo = join(dirname(fileURLToPath(import.meta.url)), "../..");
const requireFromUi = createRequire(pathToFileURL(join(repo, "crates/copypaste-ui/package.json")));
const androidNamespace = "http://schemas.android.com/apk/res/android";
const jsPlugin = "@tauri-apps/plugin-deep-link";
const rustPlugin = "tauri-plugin-deep-link";

function jsoncParser() {
  return requireFromUi("jsonc-parser");
}

function xmlDom() {
  return requireFromUi("@xmldom/xmldom");
}

function androidAttribute(element, name) {
  return element.getAttributeNS(androidNamespace, name);
}

function childElements(element, name) {
  return Array.from(element.childNodes)
    .filter((node) => node.nodeType === 1 && node.tagName === name);
}

function parseAndroidManifest(source) {
  const { DOMParser } = xmlDom();
  const errors = [];
  const document = new DOMParser({
    onError: (level, message) => errors.push(`${level}: ${message}`),
  }).parseFromString(source, "application/xml");
  if (errors.length !== 0) {
    throw new Error(`AndroidManifest.xml is not valid XML: ${errors.join("; ")}`);
  }
  return document;
}

function androidDeepLinkFilters(document) {
  return Array.from(document.getElementsByTagName("intent-filter")).filter((filter) => {
    const actions = childElements(filter, "action").map((action) => androidAttribute(action, "name"));
    const hasScheme = childElements(filter, "data").some((data) => androidAttribute(data, "scheme"));
    return actions.some((action) => action === "android.intent.action.VIEW"
      || action === "org.chromium.arc.intent.action.VIEW")
      && hasScheme;
  });
}

export function deepLinkSurfaces(configs) {
  const surfaces = [];
  const product = configs.cargoMetadata.metadata?.copypaste;
  if (product && Object.hasOwn(product, "deep-link-scheme")) surfaces.push("Cargo product metadata");
  if (configs.cargoMetadata.packages.some((item) =>
    item.dependencies?.some((dependency) => dependency.name === rustPlugin))) {
    surfaces.push("Rust plugin dependency");
  }
  if (Object.hasOwn(configs.uiPackage.dependencies ?? {}, jsPlugin)) surfaces.push("JavaScript plugin dependency");
  if (Object.hasOwn(configs.uiLock.packages?.[""]?.dependencies ?? {}, jsPlugin)
      || Object.hasOwn(configs.uiLock.packages ?? {}, `node_modules/${jsPlugin}`)) {
    surfaces.push("JavaScript plugin lock entry");
  }

  const tauri = jsoncParser().parse(configs.tauri);
  if (Object.hasOwn(tauri?.plugins ?? {}, "deep-link")) surfaces.push("Tauri plugin configuration");
  const capability = jsoncParser().parse(configs.capability);
  if (capability?.permissions?.some((permission) =>
    typeof permission === "string" && permission.startsWith("deep-link:"))) {
    surfaces.push("Tauri capability grant");
  }
  if (androidDeepLinkFilters(parseAndroidManifest(configs.androidManifest)).length !== 0) {
    surfaces.push("Android deep-link intent filter");
  }
  if (configs.androidManifest.includes("DEEP LINK PLUGIN. AUTO-GENERATED")) {
    surfaces.push("Android generated plugin block");
  }
  return surfaces;
}

export function assertNoDeepLinks(configs) {
  const surfaces = deepLinkSurfaces(configs);
  if (surfaces.length !== 0) {
    throw new Error(`deep links are not a product surface: ${surfaces.join(", ")}`);
  }
}
