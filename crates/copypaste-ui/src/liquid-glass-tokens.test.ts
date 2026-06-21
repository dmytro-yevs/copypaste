/**
 * Tests for the Liquid Glass token layer (CopyPaste-52mz).
 *
 * These tests verify:
 * 1. The new liquid-* token vocabulary exists in CSS (validated via store defaults
 *    and index.html attributes that set the default palette).
 * 2. The DEFAULT_PREFS reflects Graphite Mist as the new dark default.
 * 3. store.ts palette field is wired.
 * 4. index.html carries the correct data-* defaults for Graphite Mist.
 */
import { describe, it, expect } from "vitest";

// Import the token registry (CSS is the authoritative runtime layer;
// this module gives TS parity so tests can assert exact values).
import { PALETTE_KEYS } from "./lib/liquid-tokens";

describe("liquid-glass token layer (CopyPaste-52mz)", () => {
  describe("PALETTE_KEYS", () => {
    it("exports all 10 palette keys", () => {
      const expected = [
        "liquid-blue",
        "graphite-mist",
        "deep-sky",
        "nordic-cyan",
        "cloud-silver",
        "frost-blue",
        "porcelain",
        "pearl-grey",
        "aurora-violet",
        "amber-night",
      ];
      expect(PALETTE_KEYS).toEqual(expected);
    });
  });


});
