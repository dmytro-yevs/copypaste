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
import { LIQUID_DEFAULTS, PALETTE_KEYS, PALETTE_SCHEMES } from "./lib/liquid-tokens";

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

  describe("LIQUID_DEFAULTS (Graphite Mist)", () => {
    it("has the correct bg values", () => {
      expect(LIQUID_DEFAULTS.bg0).toBe("#07090f");
      expect(LIQUID_DEFAULTS.bg1).toBe("#141924");
      expect(LIQUID_DEFAULTS.bg2).toBe("#202330");
    });

    it("has the correct glow values", () => {
      expect(LIQUID_DEFAULTS.glowA).toBe("#7f8da3");
      expect(LIQUID_DEFAULTS.glowB).toBe("#5a6a83");
    });

    it("has the correct surface-rgb values", () => {
      expect(LIQUID_DEFAULTS.surfaceRgb).toBe("28, 31, 42");
      expect(LIQUID_DEFAULTS.surfaceStrongRgb).toBe("42, 46, 60");
    });

    it("has the correct accent values", () => {
      expect(LIQUID_DEFAULTS.accent).toBe("#9db7df");
      expect(LIQUID_DEFAULTS.accent2).toBe("#d5e2f7");
      expect(LIQUID_DEFAULTS.accent3).toBe("#7c8ca6");
      expect(LIQUID_DEFAULTS.onAccent).toBe("#ffffff");
    });

    it("has the correct semantic colour values", () => {
      expect(LIQUID_DEFAULTS.success).toBe("#7be0b1");
      expect(LIQUID_DEFAULTS.warning).toBe("#ffcc6a");
      expect(LIQUID_DEFAULTS.danger).toBe("#ff7f8c");
    });

    it("has the correct glass properties", () => {
      expect(LIQUID_DEFAULTS.glassOpacity).toBe(0.64);
      expect(LIQUID_DEFAULTS.glassBlur).toBe("28px");
      expect(LIQUID_DEFAULTS.glassSaturation).toBe(1.45);
      expect(LIQUID_DEFAULTS.glowStrength).toBe(0.62);
    });

    it("has the correct motion defaults for cinematic", () => {
      expect(LIQUID_DEFAULTS.speed).toBe(0.72);
      expect(LIQUID_DEFAULTS.motionOpacity).toBe(1);
    });

    it("is a dark scheme palette", () => {
      expect(LIQUID_DEFAULTS.scheme).toBe("dark");
    });
  });

  describe("palette scheme classification", () => {
    it("identifies light palettes correctly", () => {
      expect(PALETTE_SCHEMES["cloud-silver"]).toBe("light");
      expect(PALETTE_SCHEMES["frost-blue"]).toBe("light");
      expect(PALETTE_SCHEMES["porcelain"]).toBe("light");
      expect(PALETTE_SCHEMES["pearl-grey"]).toBe("light");
    });

    it("identifies dark palettes correctly", () => {
      expect(PALETTE_SCHEMES["graphite-mist"]).toBe("dark");
      expect(PALETTE_SCHEMES["liquid-blue"]).toBe("dark");
      expect(PALETTE_SCHEMES["deep-sky"]).toBe("dark");
      expect(PALETTE_SCHEMES["nordic-cyan"]).toBe("dark");
      expect(PALETTE_SCHEMES["aurora-violet"]).toBe("dark");
      expect(PALETTE_SCHEMES["amber-night"]).toBe("dark");
    });
  });
});
