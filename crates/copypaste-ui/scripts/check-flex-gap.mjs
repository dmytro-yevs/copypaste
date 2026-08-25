import { globSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

import flexGapPolyfill from "flex-gap-polyfill";
import postcss from "postcss";

const GAP_PROPERTIES = new Set(["gap", "row-gap", "column-gap"]);
const GENERATED_GAP_PROPERTIES = new Map([
  ["--fgp-gap", "gap"],
  ["--fgp-row-gap", "row-gap"],
  ["--fgp-column-gap", "column-gap"],
]);
const DISPLAY_FLEX = /^(?:inline-)?flex$/;
const DISPLAY_GRID = /^(?:inline-)?grid$/;

function directDeclarations(rule) {
  return rule.nodes?.filter((node) => node.type === "decl") ?? [];
}

function contextKey(rule) {
  const context = [];
  for (let node = rule.parent; node?.type === "atrule"; node = node.parent) {
    context.unshift(`@${node.name} ${node.params}`);
  }
  return context.join("|");
}

function targetKey(rule) {
  return `${contextKey(rule)}\u0000${rule.selector}`;
}

function increment(map, key) {
  map.set(key, (map.get(key) ?? 0) + 1);
}

function compareTargets(expected, actual, file, errors) {
  for (const [key, count] of expected) {
    if ((actual.get(key) ?? 0) !== count) {
      const selector = key.slice(key.indexOf("\u0000") + 1);
      errors.push(`${file}: flex gap was not transformed exactly once: ${selector}`);
    }
  }
  for (const [key, count] of actual) {
    if ((expected.get(key) ?? 0) !== count) {
      const selector = key.slice(key.indexOf("\u0000") + 1);
      errors.push(`${file}: non-flex gap received a legacy transform: ${selector}`);
    }
  }
}

export function classifySourceCss(css, file = "fixture.css") {
  const root = postcss.parse(css, { from: file });
  const facts = new Map();
  const errors = [];
  const flexTargets = new Map();
  let flexRules = 0;
  let gridRules = 0;
  let inertRules = 0;

  root.walkRules((rule) => {
    const fact = facts.get(rule.selector) ?? {
      flex: false,
      grid: false,
      unsafe: [],
    };
    for (const declaration of directDeclarations(rule)) {
      if (declaration.prop === "display") {
        fact.flex ||= DISPLAY_FLEX.test(declaration.value);
        fact.grid ||= DISPLAY_GRID.test(declaration.value);
      }
      if (
        declaration.prop.startsWith("background") ||
        declaration.prop === "margin" ||
        declaration.prop.startsWith("margin-")
      ) {
        fact.unsafe.push(`${declaration.prop}: ${declaration.value}`);
      }
    }
    facts.set(rule.selector, fact);
  });

  root.walkRules((rule) => {
    const declarations = directDeclarations(rule);
    const gaps = declarations.filter((declaration) =>
      GAP_PROPERTIES.has(declaration.prop),
    );
    if (gaps.length === 0) return;

    const comments = new Set(
      (rule.nodes ?? [])
        .filter((node) => node.type === "comment")
        .map((comment) => comment.text.trim()),
    );
    const displayValues = declarations
      .filter((declaration) => declaration.prop === "display")
      .map((declaration) => declaration.value);
    const directFlex = displayValues.some((value) => DISPLAY_FLEX.test(value));
    const directGrid = displayValues.some((value) => DISPLAY_GRID.test(value));
    const fact = facts.get(rule.selector);
    const markers = ["apply fgp", "grid gap", "inert gap"].filter((marker) =>
      comments.has(marker),
    );

    if (markers.length > 1) {
      errors.push(`${file}:${rule.source.start.line}: conflicting gap markers`);
      return;
    }

    let kind;
    if (comments.has("inert gap")) kind = "inert";
    else if (comments.has("apply fgp")) kind = "flex";
    else if (comments.has("grid gap")) kind = "grid";
    else if (directFlex) kind = "flex";
    else if (directGrid) kind = "grid";
    else if (fact.flex && !fact.grid) kind = "flex";
    else if (fact.grid && !fact.flex) kind = "grid";
    else kind = "unknown";

    if (kind === "inert") {
      inertRules += 1;
      if (gaps.some((gap) => gap.value.trim() !== "0")) {
        errors.push(`${file}:${rule.source.start.line}: inert gap must be zero`);
      }
      return;
    }

    if (kind === "grid") {
      gridRules += 1;
      if (directFlex && !directGrid) {
        errors.push(`${file}:${rule.source.start.line}: flex rule marked as grid gap`);
      }
      return;
    }

    if (kind === "unknown") {
      errors.push(
        `${file}:${rule.source.start.line}: gap needs flex, grid, or inert classification: ${rule.selector}`,
      );
      return;
    }

    flexRules += 1;
    increment(flexTargets, targetKey(rule));
    if (directGrid && !directFlex) {
      errors.push(`${file}:${rule.source.start.line}: grid rule marked for flex gap`);
    }
    if (rule.selector.includes(",")) {
      errors.push(
        `${file}:${rule.source.start.line}: flex-gap-polyfill cannot safely scope comma selectors: ${rule.selector}`,
      );
    }
    if (fact.unsafe.length > 0) {
      errors.push(
        `${file}:${rule.source.start.line}: flex-gap container needs an inner layout wrapper before using ${fact.unsafe.join(", ")}: ${rule.selector}`,
      );
    }
  });

  return { errors, flexRules, gridRules, inertRules, flexTargets };
}

export async function auditSourceCss(css, file = "fixture.css") {
  const result = classifySourceCss(css, file);
  if (result.errors.length > 0) return result;

  const transformed = await postcss([
    flexGapPolyfill({
      only: true,
      flexGapNotSupported: ".flexGapNotSupported",
    }),
  ]).process(css, { from: file });
  const actualTargets = new Map();
  transformed.root.walkRules((rule) => {
    if (
      directDeclarations(rule).some(
        (declaration) => GENERATED_GAP_PROPERTIES.has(declaration.prop),
      )
    ) {
      increment(actualTargets, targetKey(rule));
    }
  });
  compareTargets(result.flexTargets, actualTargets, file, result.errors);
  return result;
}

function selectorBranches(selector) {
  return selector.split(",").map((branch) => branch.trim());
}

function normalizedSelector(selector) {
  return selector
    .replace(/\s*([>+~])\s*/g, "$1")
    .replace(/\s+/g, " ")
    .trim();
}

export function auditBuiltCss(css, file = "dist.css") {
  const root = postcss.parse(css, { from: file });
  const errors = [];
  const contexts = new Map();
  const facts = new Map();
  let polyfilledRules = 0;

  root.walkRules((rule) => {
    const context = contextKey(rule);
    const selectors = contexts.get(context) ?? new Set();
    const selector = normalizedSelector(rule.selector);
    selectors.add(selector);
    contexts.set(context, selectors);

    const key = `${context}\u0000${selector}`;
    const fact = facts.get(key) ?? { grid: false };
    for (const declaration of directDeclarations(rule)) {
      if (declaration.prop === "display") {
        fact.grid ||= DISPLAY_GRID.test(declaration.value);
      }
    }
    facts.set(key, fact);
  });

  root.walkRules((rule) => {
    const declarations = directDeclarations(rule);
    const generatedFallback = declarations.some((declaration) =>
      ["--has-fgp", "--parent-has-fgp"].includes(declaration.prop),
    );
    if (generatedFallback && !rule.selector.startsWith(":root")) {
      for (const branch of selectorBranches(rule.selector)) {
        if (!branch.startsWith(".flexGapNotSupported")) {
          errors.push(`${file}: unscoped legacy flex-gap branch: ${branch}`);
        }
      }
    }

    const generatedGaps = declarations.filter((declaration) =>
      GENERATED_GAP_PROPERTIES.has(declaration.prop),
    );
    if (generatedGaps.length === 0) {
      return;
    }
    polyfilledRules += 1;
    const context = contextKey(rule);
    const selectors = contexts.get(context) ?? new Set();
    const selector = normalizedSelector(rule.selector);
    const prefix = ".flexGapNotSupported ";
    const required = [
      `${prefix}${selector}`,
      `${prefix}${selector}>*`,
      `${prefix}${selector}>*>*`,
    ];
    for (const selector of required) {
      if (!selectors.has(selector)) {
        errors.push(`${file}: missing scoped legacy fallback ${selector}`);
      }
    }
    if (facts.get(`${context}\u0000${selector}`)?.grid) {
      errors.push(`${file}: grid selector was contaminated by flex-gap output: ${rule.selector}`);
    }
    for (const generated of generatedGaps) {
      const property = GENERATED_GAP_PROPERTIES.get(generated.prop);
      const fallback = `var(${generated.prop},0px)`;
      if (
        !declarations.some(
          (declaration) =>
            declaration.prop === property &&
            declaration.value.replace(/\s+/g, "") === fallback,
        )
      ) {
        errors.push(
          `${file}: transformed rule lost its ${property} legacy fallback: ${rule.selector}`,
        );
      }
    }
  });

  if (css.includes(":global(")) {
    errors.push(`${file}: emitted CSS contains a CSS Modules :global() artifact`);
  }
  return { errors, polyfilledRules };
}

function assertWiring(errors) {
  const config = readFileSync("vite.config.ts", "utf8");
  const main = readFileSync("src/main.tsx", "utf8");
  const detector = readFileSync("src/lib/flexGapSupport.ts", "utf8");
  const detectorTest = readFileSync("src/lib/flexGapSupport.test.ts", "utf8");

  if (!/only:\s*true/.test(config) || /only:\s*\[/.test(config)) {
    errors.push("vite.config.ts: flex-gap polyfill must use only: true without a selector allowlist");
  }
  if (!/flexGapNotSupported:\s*["']\.flexGapNotSupported["']/.test(config)) {
    errors.push("vite.config.ts: legacy selector does not match the detector class");
  }
  const probe = main.indexOf(
    "applyFlexGapSupportState(document, flexGapQaForcesUnsupported());",
  );
  const render = main.indexOf("createRoot(root");
  if (probe < 0 || render < 0 || probe > render) {
    errors.push("src/main.tsx: flex-gap probe must run before createRoot");
  }
  for (const requirement of [
    'style.display = "flex"',
    'style.rowGap = "1px"',
    "scrollHeight",
    "appendChild(probe)",
    "probe.remove()",
  ]) {
    if (!detector.includes(requirement)) {
      errors.push(`src/lib/flexGapSupport.ts: missing layout-probe step ${requirement}`);
    }
  }
  if (detector.includes("CSS.supports")) {
    errors.push("src/lib/flexGapSupport.ts: CSS.supports is not a flex-gap layout probe");
  }
  if (!detector.includes("import.meta.env.DEV")) {
    errors.push("src/lib/flexGapSupport.ts: QA override must be compile-time DEV-only");
  }
  if (!detectorTest.includes("mockReturnValueOnce(2)")) {
    errors.push("src/lib/flexGapSupport.test.ts: Safari 14 two-pixel probe path is untested");
  }
}

export async function run({ source = true, dist = true } = {}) {
  const errors = [];
  let flexRules = 0;
  let gridRules = 0;
  let inertRules = 0;
  let emittedRules = 0;

  if (source) {
    assertWiring(errors);
    for (const file of globSync("src/**/*.css")) {
      const result = await auditSourceCss(readFileSync(file, "utf8"), file);
      errors.push(...result.errors);
      flexRules += result.flexRules;
      gridRules += result.gridRules;
      inertRules += result.inertRules;
    }
  }

  if (dist) {
    const files = globSync("dist/assets/*.css");
    if (files.length === 0) errors.push("dist/assets: no emitted CSS to audit");
    for (const file of files) {
      const result = auditBuiltCss(readFileSync(file, "utf8"), file);
      errors.push(...result.errors);
      emittedRules += result.polyfilledRules;
    }
    if (emittedRules === 0) errors.push("dist/assets: no emitted flex-gap fallbacks found");

    const detector = readFileSync("src/lib/flexGapSupport.ts", "utf8");
    const query = detector.match(/FLEX_GAP_QA_QUERY\s*=\s*["']([^"']+)["']/)?.[1];
    if (!query) {
      errors.push("src/lib/flexGapSupport.ts: missing centralized QA query name");
    } else {
      for (const file of globSync("dist/**/*.js")) {
        if (readFileSync(file, "utf8").includes(query)) {
          errors.push(`${file}: DEV-only flex-gap QA query leaked into production`);
        }
      }
    }
  }

  if (errors.length > 0) throw new Error(errors.join("\n"));
  return { flexRules, gridRules, inertRules, emittedRules };
}

const isMain = process.argv[1] &&
  resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url));
if (isMain) {
  const sourceOnly = process.argv.includes("--source");
  const distOnly = process.argv.includes("--dist");
  const result = await run({
    source: sourceOnly || !distOnly,
    dist: distOnly || !sourceOnly,
  });
  console.log(
    `Flex gap: ${result.flexRules} source flex, ${result.gridRules} grid, ${result.inertRules} inert, ${result.emittedRules} emitted fallbacks`,
  );
}
