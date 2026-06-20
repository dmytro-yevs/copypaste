import { describe, it, expect } from "vitest";
import { SKIN_IDS, SKINS, type SkinId, type SkinTokens } from "./skins";

// All three skin IDs must be present
describe("SKIN_IDS", () => {
  it("contains all three skin identifiers", () => {
    expect(SKIN_IDS).toContain("classic");
    expect(SKIN_IDS).toContain("quiet");
    expect(SKIN_IDS).toContain("vapor");
    expect(SKIN_IDS).toHaveLength(3);
  });
});

// Every SkinTokens key must be present in each skin bundle
const REQUIRED_KEYS: (keyof SkinTokens)[] = [
  "material",
  "glassBlur",
  "saturation",
  "fillAlpha",
  "sheen",
  "tintAlpha",
  "elevation",
  "shadowCard",
  "shadowFloat",
  "radiusControl",
  "radiusChip",
  "radiusCard",
  "radiusModal",
  "rowTreatment",
  "rowGap",
  "navActive",
  "background",
  "glow",
  "motionScale",
  // CopyPaste-fuxf (M5): strong-tier blur token
  "glassBlurStrong",
  // CopyPaste-0kbq (M2): light-mode sheen token
  "sheenLight",
];

describe("SKINS registry completeness", () => {
  for (const id of ["classic", "quiet", "vapor"] as SkinId[]) {
    it(`${id} has every SkinTokens field`, () => {
      const skin = SKINS[id];
      for (const key of REQUIRED_KEYS) {
        expect(skin).toHaveProperty(key);
      }
    });
  }
});

// Classic numeric values must match §2.2 table exactly (frozen, single source of truth)
describe("SKINS.classic — frozen values match §2.2", () => {
  const c = SKINS.classic;

  it("material is 'glass'", () => expect(c.material).toBe("glass"));
  it("glassBlur is 28", () => expect(c.glassBlur).toBe(28));
  it("saturation is 1.45", () => expect(c.saturation).toBe(1.45));
  // CopyPaste-yvor: reconciled to 0.40 (matches CSS --skin-fill:.40 and :root --glass-opacity:.40)
  it("fillAlpha is 0.40", () => expect(c.fillAlpha).toBe(0.40));
  it("sheen is 0.06", () => expect(c.sheen).toBe(0.06));
  it("tintAlpha is 0", () => expect(c.tintAlpha).toBe(0));
  it("elevation is 'glass-float'", () => expect(c.elevation).toBe("glass-float"));
  it("shadowCard is 'e2'", () => expect(c.shadowCard).toBe("e2"));
  it("shadowFloat is 'e3'", () => expect(c.shadowFloat).toBe("e3"));
  it("radiusControl is 9", () => expect(c.radiusControl).toBe(9));
  it("radiusChip is 7", () => expect(c.radiusChip).toBe(7));
  it("radiusCard is 14", () => expect(c.radiusCard).toBe(14));
  it("radiusModal is 16", () => expect(c.radiusModal).toBe(16));
  it("rowTreatment is 'card'", () => expect(c.rowTreatment).toBe("card"));
  it("rowGap is 0", () => expect(c.rowGap).toBe(0));
  it("navActive is 'fill-glow'", () => expect(c.navActive).toBe("fill-glow"));
  it("background is 'aurora'", () => expect(c.background).toBe("aurora"));
  it("glow is 0.62", () => expect(c.glow).toBe(0.62));
  it("motionScale is 1.3", () => expect(c.motionScale).toBe(1.3));
});

// Quiet spot-checks
describe("SKINS.quiet — key values from §2.2", () => {
  const q = SKINS.quiet;

  it("material is 'flat'", () => expect(q.material).toBe("flat"));
  it("glassBlur is 0", () => expect(q.glassBlur).toBe(0));
  it("saturation is 1.0", () => expect(q.saturation).toBe(1.0));
  it("fillAlpha is 1.0", () => expect(q.fillAlpha).toBe(1.0));
  it("sheen is 0", () => expect(q.sheen).toBe(0));
  it("elevation is 'none'", () => expect(q.elevation).toBe("none"));
  it("shadowCard is 'none'", () => expect(q.shadowCard).toBe("none"));
  it("shadowFloat is 'e1'", () => expect(q.shadowFloat).toBe("e1"));
  it("radiusControl is 7", () => expect(q.radiusControl).toBe(7));
  it("rowTreatment is 'line'", () => expect(q.rowTreatment).toBe("line"));
  it("navActive is 'tint'", () => expect(q.navActive).toBe("tint"));
  it("background is 'flat'", () => expect(q.background).toBe("flat"));
  it("glow is 0", () => expect(q.glow).toBe(0));
  it("motionScale is 1.0", () => expect(q.motionScale).toBe(1.0));
});

// Vapor spot-checks
describe("SKINS.vapor — key values from §2.2", () => {
  const v = SKINS.vapor;

  it("material is 'glass'", () => expect(v.material).toBe("glass"));
  it("glassBlur is 34", () => expect(v.glassBlur).toBe(34));
  it("saturation is 1.7", () => expect(v.saturation).toBe(1.7));
  it("fillAlpha is 0.50", () => expect(v.fillAlpha).toBe(0.5));
  it("sheen is 0.16", () => expect(v.sheen).toBe(0.16));
  it("tintAlpha is 0.14", () => expect(v.tintAlpha).toBe(0.14));
  it("elevation is 'glass-float'", () => expect(v.elevation).toBe("glass-float"));
  it("shadowCard is 'none'", () => expect(v.shadowCard).toBe("none"));
  it("shadowFloat is 'e3'", () => expect(v.shadowFloat).toBe("e3"));
  it("radiusControl is 12", () => expect(v.radiusControl).toBe(12));
  it("radiusChip is 10", () => expect(v.radiusChip).toBe(10));
  it("radiusCard is 16", () => expect(v.radiusCard).toBe(16));
  it("radiusModal is 16", () => expect(v.radiusModal).toBe(16));
  it("rowTreatment is 'inset'", () => expect(v.rowTreatment).toBe("inset"));
  it("rowGap is 3", () => expect(v.rowGap).toBe(3));
  it("navActive is 'glass-ring'", () => expect(v.navActive).toBe("glass-ring"));
  it("background is 'tint-blob'", () => expect(v.background).toBe("tint-blob"));
  it("glow is 0.45", () => expect(v.glow).toBe(0.45));
  it("motionScale is 1.0", () => expect(v.motionScale).toBe(1.0));
});

// ---------------------------------------------------------------------------
// CopyPaste-yvor (M3): fillAlpha reconciliation
// Classic CSS renders --skin-fill:.40 (= :root --glass-opacity .40).
// skins.ts must match the rendered value so JS consumers see 0.40.
// ---------------------------------------------------------------------------
describe("CopyPaste-yvor — classic fillAlpha reconciled to rendered 0.40", () => {
  it("classic fillAlpha is 0.40 (matches CSS --skin-fill and :root --glass-opacity)", () => {
    expect(SKINS.classic.fillAlpha).toBe(0.40);
  });
});

// ---------------------------------------------------------------------------
// CopyPaste-fuxf (M5): glassBlurStrong token
// ---------------------------------------------------------------------------
describe("CopyPaste-fuxf — glassBlurStrong token in SkinTokens", () => {
  it("classic glassBlurStrong is 40 (px, matches legacy hardcoded value)", () => {
    expect(SKINS.classic.glassBlurStrong).toBe(40);
  });
  it("quiet glassBlurStrong is 0 (flat skin — no strong blur)", () => {
    expect(SKINS.quiet.glassBlurStrong).toBe(0);
  });
  it("vapor glassBlurStrong is 44 (deeper frosted modals)", () => {
    expect(SKINS.vapor.glassBlurStrong).toBe(44);
  });
  it("SkinTokens interface includes glassBlurStrong as required key", () => {
    // TypeScript compile check: every skin bundle must have the key
    const requiredKeys: (keyof SkinTokens)[] = ["glassBlurStrong"];
    for (const id of SKIN_IDS) {
      for (const key of requiredKeys) {
        expect(SKINS[id]).toHaveProperty(key);
      }
    }
  });
});

// ---------------------------------------------------------------------------
// CopyPaste-0kbq (M2): sheenLight token (light-mode sheen)
// classic=0.45, quiet=0, vapor=0.70
// ---------------------------------------------------------------------------
describe("CopyPaste-0kbq — sheenLight token in SkinTokens", () => {
  it("classic sheenLight is 0.45", () => {
    expect(SKINS.classic.sheenLight).toBe(0.45);
  });
  it("quiet sheenLight is 0", () => {
    expect(SKINS.quiet.sheenLight).toBe(0);
  });
  it("vapor sheenLight is 0.70 (matches CSS html[data-skin=vapor][data-theme=light]{--skin-sheen:.70})", () => {
    expect(SKINS.vapor.sheenLight).toBe(0.70);
  });
  it("SkinTokens interface includes sheenLight as required key", () => {
    const requiredKeys: (keyof SkinTokens)[] = ["sheenLight"];
    for (const id of SKIN_IDS) {
      for (const key of requiredKeys) {
        expect(SKINS[id]).toHaveProperty(key);
      }
    }
  });
});
