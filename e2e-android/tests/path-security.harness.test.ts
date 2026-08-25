import path from "node:path";

import { describe, expect, test } from "vitest";

import {
  ANDROID_PATH_LEAK_PATTERNS,
  DESKTOP_PATH_LEAK_PATTERNS,
  findFilesystemPathLeaks,
  PATH_SECURITY_VECTORS,
} from "../../test-support/security/path-leaks.js";

describe("shared filesystem-path policy", () => {
  test.each(PATH_SECURITY_VECTORS.displayLeakCases)(
    "rejects $id",
    ({ platform, surface, expectedRule }) => {
      const additions =
        platform === "android" ? ANDROID_PATH_LEAK_PATTERNS : DESKTOP_PATH_LEAK_PATTERNS;
      const rules = findFilesystemPathLeaks(surface, { additions }).map((leak) => leak.rule);
      expect(rules).toContain(expectedRule);
    },
  );

  test.each(PATH_SECURITY_VECTORS.safeDisplayCases)("accepts $id", ({ surface }) => {
    expect(findFilesystemPathLeaks(surface, { additions: DESKTOP_PATH_LEAK_PATTERNS })).toEqual([]);
    expect(findFilesystemPathLeaks(surface, { additions: ANDROID_PATH_LEAK_PATTERNS })).toEqual([]);
  });

  test("carries every alias and containment attack shape", () => {
    const cases = new Map(
      PATH_SECURITY_VECTORS.artifactContainmentCases.map((entry) => [entry.kind, entry]),
    );
    expect([...cases.keys()].sort()).toEqual([
      "case_collision",
      "duplicate_resolved",
      "hardlink_alias",
      "symlink_leaf",
      "symlink_parent",
      "traversal",
    ]);

    const duplicate = cases.get("duplicate_resolved");
    expect(duplicate).toBeDefined();
    expect(new Set(duplicate?.entries.map((entry) => path.posix.normalize(entry))).size).toBe(1);

    const collision = cases.get("case_collision");
    expect(collision).toBeDefined();
    expect(new Set(collision?.entries.map((entry) => entry.toLocaleLowerCase("en-US"))).size).toBe(1);

    for (const entry of cases.values()) expect(entry.expected).toBe("reject");
  });
});
