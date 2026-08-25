/**
 * Each emitted chunk must run on the engine that will be asked to load it: the
 * module build on API 29's Chromium 74, the nomodule build on API 24's
 * Chromium 53 (`webview-baseline.mjs` records where both numbers come from).
 *
 * Which syntax each engine lacks is `@babel/compat-data`'s answer, not one
 * written down here — this file maps a Babel transform to the AST it exists to
 * lower and asks that package for the version. `--self-test` proves each map
 * entry fires.
 */
import { globSync, readFileSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { parse } from "@babel/parser";
import _traverse from "@babel/traverse";
import { parse as parseCss } from "postcss";

// `require`, because the package resolves this to `plugins.js` or to
// `data/plugins.json` depending on how it was installed, and the two need
// different import syntax.
const compatData = createRequire(import.meta.url)("@babel/compat-data/plugins");

import { unreferenced } from "./orphan-chunks.mjs";
import { LEGACY_TARGET, MODERN_TARGET } from "./webview-baseline.mjs";

const traverse = _traverse.default ?? _traverse;
const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");

/**
 * Babel transform -> the AST it lowers. Structure, not compatibility: every
 * version below comes from `@babel/compat-data`.
 */
const LOWERED = {
  "transform-optional-chaining": (note) => ({
    OptionalMemberExpression: note,
    OptionalCallExpression: note,
  }),
  "transform-nullish-coalescing-operator": (note) => ({
    LogicalExpression: (p) => p.node.operator === "??" && note(p),
  }),
  "transform-logical-assignment-operators": (note) => ({
    AssignmentExpression: (p) => ["||=", "&&=", "??="].includes(p.node.operator) && note(p),
  }),
  "transform-private-methods": (note) => ({ ClassPrivateMethod: note }),
  "transform-class-static-block": (note) => ({ StaticBlock: note }),
  "transform-numeric-separator": (note) => ({
    NumericLiteral: (p) => (p.node.extra?.raw ?? "").includes("_") && note(p),
  }),
  "transform-async-to-generator": (note) => ({
    AwaitExpression: note,
    Function: (p) => p.node.async === true && note(p),
  }),
  "transform-object-rest-spread": (note) => ({
    ObjectExpression: (p) => p.node.properties.some((x) => x.type === "SpreadElement") && note(p),
    ObjectPattern: (p) => p.node.properties.some((x) => x.type === "RestElement") && note(p),
  }),
  "transform-class-properties": (note) => ({ ClassProperty: note, ClassPrivateProperty: note }),
  "transform-exponentiation-operator": (note) => ({
    BinaryExpression: (p) => p.node.operator === "**" && note(p),
  }),
};

const chromeOf = (target) => Number(target.replace(/^chrome/, ""));

/** The transforms `@babel/compat-data` says this engine still needs. */
export function loweredFor(target) {
  const engine = chromeOf(target);
  return Object.keys(LOWERED).filter((name) => {
    const since = compatData[name]?.chrome;
    return since !== undefined && engine < Number(since);
  });
}

export function postBaselineSyntax(code, target, filename = "<input>") {
  const found = [];
  const visitor = {};
  for (const name of loweredFor(target)) {
    const since = compatData[name].chrome;
    const note = (nodePath) => {
      found.push({
        file: filename,
        line: nodePath.node.loc?.start.line ?? 0,
        what: `${name.replace(/^transform-/, "")} (Chromium ${since})`,
      });
    };
    for (const [type, run] of Object.entries(LOWERED[name](note))) {
      const previous = visitor[type];
      visitor[type] = (nodePath) => {
        previous?.(nodePath);
        run(nodePath);
      };
    }
  }

  traverse(
    parse(code, { sourceType: "unambiguous", allowAwaitOutsideFunction: true }),
    visitor,
  );
  return found;
}

/**
 * Lowering syntax cannot add a method, and tsconfig's pinned `lib` only sees
 * our own source, so a dependency's call is invisible to both. core-js covers
 * the builtins for the legacy build; what is left is DOM, which it does not
 * carry. An entry with no `polyfill` is a gap, not an exemption.
 *
 * Matched literally, so only names no library plausibly defines itself belong
 * here. `at` and `toSorted` are the counter-example: `@dnd-kit`'s collection
 * class defines both, and matching them reported its methods as browser APIs.
 */
const RUNTIME_APIS = [
  {
    used: ".replaceChildren(",
    api: "ParentNode.replaceChildren",
    chrome: 86,
    by: "@dnd-kit/dom, on the drag placeholder",
    polyfill: "@ungap/replace-children",
    proof: "Node.prototype.replaceChildren",
  },
];

export function postBaselineRuntime(code, target) {
  const engine = chromeOf(target);
  return RUNTIME_APIS.filter((e) => e.chrome > engine && code.includes(e.used)).map((entry) => ({
    ...entry,
    covered: entry.polyfill !== undefined && code.includes(entry.proof),
  }));
}

/**
 * `-legacy-` is `@vitejs/plugin-legacy`'s naming for the nomodule build. A
 * script outside `assets/` was copied from `public/`, so the plugin never saw
 * it and both engines load it: its floor is the older one.
 */
export function targetOf(name, fromPublic = false) {
  if (fromPublic) return LEGACY_TARGET;
  return /-legacy-|^polyfills-legacy/.test(name) ? LEGACY_TARGET : MODERN_TARGET;
}

/**
 * What the nomodule build must carry, named by the chunk that carries it.
 * Asserting each builtin by name would re-implement `core-js-compat`, which is
 * what decides them from the same browserslist target; asserting the chunks
 * exist and are reachable is the part that can silently stop being true.
 */
const LEGACY_CHUNKS = [
  ["polyfills-legacy", "the core-js builtins @vitejs/plugin-legacy derives from the target"],
  ["legacyPolyfills-legacy", "ResizeObserver, AbortController and Intl.RelativeTimeFormat"],
];

export function missingLegacyChunks(names) {
  return LEGACY_CHUNKS.filter(([prefix]) => !names.some((name) => name.startsWith(prefix)));
}

/** Both engines have to come from one place or the config and this check drift
 *  apart; the config is read rather than trusted to be importing them. */
export function sharesBaselineWithConfig(config) {
  return /from\s+"\.\/scripts\/webview-baseline\.mjs"/.test(config);
}

/** Shared CSS is loaded by both script variants, so its target is API 24. */
export function sharesCssBaselineWithConfig(config) {
  return (
    /import\s*\{[^}]*\bLEGACY_CSS_TARGET\b[^}]*\}\s*from\s*"\.\/scripts\/webview-baseline\.mjs"/s.test(
      config,
    ) && /\bcssTarget\s*:\s*LEGACY_CSS_TARGET\b/.test(config)
  );
}

export function unflattenedCascadeLayers(css, filename = "<input>") {
  const found = [];
  parseCss(css, { from: filename }).walkAtRules("layer", (rule) => {
    found.push({ file: filename, line: rule.source?.start?.line ?? 0 });
  });
  return found;
}

function scripts(dir, fromPublic) {
  return readdirSync(path.join(root, "dist", ...dir), { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith(".js"))
    .map(({ name }) => ({
      name,
      target: targetOf(name, fromPublic),
      code: readFileSync(path.join(root, "dist", ...dir, name), "utf8"),
    }));
}

/** Everything the packaged frontend ships, including what `public/` contributed. */
function bundles() {
  return [...scripts(["assets"], false), ...scripts([], true)];
}

function stylesheets() {
  const dist = path.join(root, "dist");
  return globSync("**/*.css", { cwd: dist }).map((name) => ({
    name,
    code: readFileSync(path.join(dist, name), "utf8"),
  }));
}

function main() {
  const config = readFileSync(path.join(root, "vite.config.ts"), "utf8");
  if (!sharesBaselineWithConfig(config)) {
    console.error("FAIL vite.config.ts does not take its engines from scripts/webview-baseline.mjs");
    return 1;
  }
  if (!sharesCssBaselineWithConfig(config)) {
    console.error("FAIL vite.config.ts does not target shared CSS at LEGACY_CSS_TARGET");
    return 1;
  }

  let chunks = [];
  let entryHtml = "";
  try {
    chunks = bundles();
    entryHtml = readFileSync(path.join(root, "dist", "index.html"), "utf8");
  } catch {
    chunks = [];
  }
  if (chunks.length === 0) {
    console.error("FAIL dist/assets holds no built bundle; run vite build first");
    return 1;
  }

  // Unreachable is not harmless: an orphan is still packaged, still shipped and
  // still downloaded with the app. `vite.config.ts` removes the one the two
  // outputs are known to leave behind, so anything left here is a new one.
  let failed = 0;
  const css = stylesheets();
  if (css.length === 0) {
    console.error("FAIL dist holds no built stylesheet; run vite build first");
    failed += 1;
  }
  const layers = css.flatMap(({ name, code }) => unflattenedCascadeLayers(code, name));
  for (const { file, line } of layers) {
    console.error(`FAIL ${file}:${line} still contains @layer; API 24 and API 29 discard it`);
  }
  failed += layers.length;
  if (css.length > 0 && layers.length === 0) {
    console.log(`ok   ${css.length} stylesheet(s) have no unsupported @layer rules`);
  }

  for (const name of unreferenced(chunks, entryHtml)) {
    console.error(`FAIL ${name} is packaged and nothing loads it; the build must not emit it`);
    failed += 1;
  }

  const byTarget = [MODERN_TARGET, LEGACY_TARGET].map((target) => ({
    target,
    chunks: chunks.filter((chunk) => chunk.target === target),
  }));
  for (const { target, chunks: forTarget } of byTarget) {
    if (forTarget.length === 0) {
      console.error(`FAIL nothing reachable was emitted for ${target}; both builds must ship`);
      return 1;
    }
  }

  const legacyNames = byTarget.find(({ target }) => target === LEGACY_TARGET)?.chunks ?? [];
  for (const [prefix, what] of missingLegacyChunks(legacyNames.map(({ name }) => name))) {
    console.error(`FAIL ${LEGACY_TARGET}: no reachable ${prefix}* chunk, so ${what} never load`);
    failed += 1;
  }

  for (const { target, chunks: forTarget } of byTarget) {
    // Whole-build, not per chunk: a polyfill is a side effect the entry runs
    // once, so it covers a call that code-splitting put in another chunk.
    const all = forTarget.map(({ code }) => code).join("\n");
    for (const entry of postBaselineRuntime(all, target)) {
      const where = forTarget.filter((c) => c.code.includes(entry.used)).map((c) => c.name);
      if (entry.covered) {
        console.log(`ok   ${target}: ${entry.api} (Chromium ${entry.chrome}) behind ${entry.polyfill}`);
        continue;
      }
      failed += 1;
      console.error(
        `FAIL ${target}: ${where.join(", ")} calls ${entry.api}, introduced in Chromium ` +
          `${entry.chrome} and reached by ${entry.by}` +
          (entry.polyfill ? `; ${entry.polyfill} is not in this build` : "; nothing polyfills it"),
      );
    }

    const found = forTarget.flatMap(({ name, code }) => postBaselineSyntax(code, target, name));
    for (const { file, line, what } of found.slice(0, 10)) {
      console.error(`FAIL ${target}: ${file}:${line} uses ${what}`);
    }
    failed += found.length;
    if (found.length === 0) {
      console.log(`ok   ${target}: ${forTarget.length} chunk(s) parse, ${loweredFor(target).length} transforms enforced`);
    }
  }
  return failed ? 1 : 0;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exit(main());
