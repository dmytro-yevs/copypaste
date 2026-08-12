import { describe, expect, test } from "vitest";

import { fixtureMarker, secretFor } from "../src/harness/fixtures.js";
import { redactFixtures, redactFromEvidence } from "../src/harness/redact.js";

describe("fixture markers", () => {
  test("do not contain card-shaped wall-clock numbers", () => {
    const hostedFailure = `settings-${1786494647418}`;

    expect(hostedFailure).toMatch(/[0-9]{13,19}/);
    expect(fixtureMarker("settings", 1786494647418)).not.toMatch(/[0-9]{13,19}/);
  });
});

describe("failure evidence", () => {
  test("never publishes a seeded secret, revealed or not", () => {
    const secret = secretFor("000000042");
    redactFromEvidence(secret);

    const written = redactFixtures(
      JSON.stringify({ rows: [{ text: `revealed ${secret} tail` }, { text: "ordinary" }] }),
    );

    expect(written).not.toContain(secret);
    expect(written).toContain("[redacted fixture]");
    expect(written).toContain("ordinary");
  });
});
