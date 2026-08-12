import { describe, expect, test } from "vitest";

import { fixtureMarker } from "../src/harness/fixtures.js";

describe("fixture markers", () => {
  test("do not contain card-shaped wall-clock numbers", () => {
    const hostedFailure = `settings-${1786494647418}`;

    expect(hostedFailure).toMatch(/[0-9]{13,19}/);
    expect(fixtureMarker("settings", 1786494647418)).not.toMatch(/[0-9]{13,19}/);
  });
});
