import assert from "node:assert/strict";
import test from "node:test";

import flexGapPolyfill from "flex-gap-polyfill";
import postcss from "postcss";

import {
  auditBuiltCss,
  auditSourceCss,
  classifySourceCss,
} from "./check-flex-gap.mjs";

test("classifies flex, grid, and inert gaps without a selector allowlist", async () => {
  const css = `
    .flex { display: flex; gap: var(--s-2); }
    .grid { display: grid; gap: var(--s-2); }
    .runtime { gap: 0; /* inert gap */ }
  `;
  const result = await auditSourceCss(css);

  assert.deepEqual(result.errors, []);
  assert.equal(result.flexRules, 1);
  assert.equal(result.gridRules, 1);
  assert.equal(result.inertRules, 1);
});

test("rejects package limitation cases before they alter layout", () => {
  const comma = classifySourceCss(
    ".one, .two { display: flex; gap: var(--s-2); }",
  );
  const background = classifySourceCss(
    ".unsafe { display: flex; gap: var(--s-2); background: var(--card); }",
  );

  assert.match(comma.errors.join("\n"), /comma selectors/);
  assert.match(background.errors.join("\n"), /inner layout wrapper/);
});

test("requires every emitted fallback branch to use the detector class", async () => {
  const transformed = await postcss([
    flexGapPolyfill({
      only: true,
      flexGapNotSupported: ".flexGapNotSupported",
    }),
  ]).process(".valid { display: flex; gap: 1rem; }", {
    from: "fixture.css",
  });
  assert.deepEqual(auditBuiltCss(transformed.css).errors, []);

  const unscoped = transformed.css.replaceAll(".flexGapNotSupported ", "");
  assert.match(auditBuiltCss(unscoped).errors.join("\n"), /legacy fallback/);
});
