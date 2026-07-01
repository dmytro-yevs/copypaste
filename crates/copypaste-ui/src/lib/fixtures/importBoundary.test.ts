import { readFileSync, readdirSync, statSync } from "node:fs";
import { extname, join, relative } from "node:path";
import { describe, expect, it } from "vitest";

// ---------------------------------------------------------------------------
// Import-boundary rule (design.md Decision 7/G3, round-5 M2, task 6.5):
// production code MUST NOT import from `src/lib/fixtures/**`. The only
// allowed consumers are `src/lib/mockIpc.ts` and `src/views/GalleryView/**`
// (both already DEV-gated dynamic imports — see fixtures/index.ts header).
//
// This is the `rg`/lint-rule half of the enforcement; the production build's
// chunk-graph reachability check (task 6.12) is the other half.
// ---------------------------------------------------------------------------

const SRC_ROOT = join(process.cwd(), "src");

const ALLOWED_IMPORTER_PATTERNS = [
  /^src\/lib\/mockIpc\.ts$/,
  /^src\/views\/GalleryView\//,
];

// Match any relative/aliased import specifier that resolves into lib/fixtures,
// e.g. "./fixtures", "../lib/fixtures", "../../lib/fixtures/device". A file
// living INSIDE lib/fixtures importing a sibling module (e.g. index.ts
// importing ./device) is not a boundary crossing and is intentionally excluded
// by the walk below (we skip files under lib/fixtures entirely).
const FIXTURES_IMPORT_RE = /from\s+["']([^"']*\blib\/fixtures(?:\/[^"']*)?)["']/g;

function walk(dir: string, out: string[] = []): string[] {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) {
      walk(full, out);
    } else if ([".ts", ".tsx"].includes(extname(full))) {
      out.push(full);
    }
  }
  return out;
}

describe("src/lib/fixtures import boundary", () => {
  it("is imported ONLY by mockIpc.ts and GalleryView/**", () => {
    const violations: string[] = [];

    for (const file of walk(SRC_ROOT)) {
      const relPath = relative(process.cwd(), file).replace(/\\/g, "/");

      // Skip fixtures' own internal sibling imports (e.g. index.ts -> ./device)
      // — those aren't a boundary crossing.
      if (relPath.startsWith("src/lib/fixtures/")) continue;

      const contents = readFileSync(file, "utf8");
      const isAllowed = ALLOWED_IMPORTER_PATTERNS.some((re) => re.test(relPath));
      if (isAllowed) continue;

      for (const m of contents.matchAll(FIXTURES_IMPORT_RE)) {
        violations.push(`${relPath} imports "${m[1]}"`);
      }
    }

    expect(violations).toEqual([]);
  });

  it("guards against a no-op regex (fails loudly if extraction breaks)", () => {
    // Positive-control sample text — proves FIXTURES_IMPORT_RE actually matches
    // the shape of import we care about, so the assertion above isn't
    // vacuously passing because the regex stopped matching anything.
    const sample = 'import { makeDevice } from "../../lib/fixtures";';
    const matches = [...sample.matchAll(FIXTURES_IMPORT_RE)];
    expect(matches).toHaveLength(1);
    expect(matches[0][1]).toBe("../../lib/fixtures");
  });
});
