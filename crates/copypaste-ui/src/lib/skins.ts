/**
 * Skin axis — visual language registry (Classic / Quiet / Vapor).
 *
 * This is the SINGLE SOURCE OF TRUTH for skin tokens.
 * Android's `ui/theme/Skin.kt` mirrors these field names verbatim (CI parity check).
 *
 * Skin is orthogonal to theme (light/dark) and palette (accent color).
 * Classic reproduces the current Liquid Glass look; do NOT change its values.
 *
 * Adding a new skin: add one entry here + one html[data-skin="…"] block in index.css.
 * No component files need to be touched.
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type SkinId = "classic" | "quiet" | "vapor";

export const SKIN_IDS: SkinId[] = ["classic", "quiet", "vapor"];

/** Surface material: glass = frosted/translucent, flat = opaque solid. */
export type Material = "glass" | "flat";

/** Elevation model. */
export type Elevation = "glass-float" | "none";

/** Shadow token references (e-scale). */
export type ShadowToken = "e1" | "e2" | "e3" | "none";

/** How list rows are rendered. */
export type RowTreatment = "card" | "line" | "inset";

/** Nav active-item indicator style. */
export type NavActive = "fill-glow" | "tint" | "glass-ring";

/** Background mode. */
export type Background = "aurora" | "flat" | "tint-blob";

/**
 * SkinTokens — all ~19 structural tokens for one skin.
 *
 * Field names must match Android Skin.kt exactly (CI enforced).
 * Use numeric types for dimensionless scalars and pixel values.
 * Use string-union types for enum-like choices.
 */
export interface SkinTokens {
  /** Surface material: 'glass' | 'flat'. */
  material: Material;

  /** Glass blur radius in px (0 = no blur / flat). */
  glassBlur: number;

  /** Backdrop saturation multiplier (1.0 = identity). */
  saturation: number;

  /** Surface fill opacity (0–1). For glass: the frosted fill alpha; for flat: solid = 1.0. */
  fillAlpha: number;

  /** Inner sheen/highlight alpha on glass surfaces (0 = none). */
  sheen: number;

  /** Accent-tint wash alpha layered over the surface (0 = none). */
  tintAlpha: number;

  /** Elevation / shadow model: 'glass-float' | 'none'. */
  elevation: Elevation;

  /** Card-level shadow token reference: 'e1'|'e2'|'e3'|'none'. */
  shadowCard: ShadowToken;

  /** Floating element shadow token reference: 'e1'|'e2'|'e3'|'none'. */
  shadowFloat: ShadowToken;

  /** Border-radius for controls (buttons, inputs) in px. */
  radiusControl: number;

  /** Border-radius for chips / tags in px. */
  radiusChip: number;

  /** Border-radius for cards in px. */
  radiusCard: number;

  /** Border-radius for modals / sheets in px. */
  radiusModal: number;

  /** List row visual treatment: 'card' | 'line' | 'inset'. */
  rowTreatment: RowTreatment;

  /** Vertical gap between rows in px (0 = flush). */
  rowGap: number;

  /** Nav active-item indicator: 'fill-glow' | 'tint' | 'glass-ring'. */
  navActive: NavActive;

  /** Background rendering mode: 'aurora' | 'flat' | 'tint-blob'. */
  background: Background;

  /** Aurora / accent glow strength (0–1, 0 = off). */
  glow: number;

  /** Motion duration multiplier (1.0 = default; 1.3 = cinematic/slower). */
  motionScale: number;
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/**
 * SKINS — the three canonical skin bundles.
 *
 * Values come from §2.2 of docs/design/skins-implementation-plan.md.
 * Classic values are frozen to the current Liquid Glass look (byte-identical to today).
 */
export const SKINS: Record<SkinId, SkinTokens> = {
  /**
   * Classic — current Liquid Glass look, frozen.
   * Default skin; changing these values is NOT allowed.
   */
  classic: {
    material: "glass",
    glassBlur: 28,
    saturation: 1.45,
    fillAlpha: 0.62,
    sheen: 0.06,
    tintAlpha: 0,
    elevation: "glass-float",
    shadowCard: "e2",
    shadowFloat: "e3",
    radiusControl: 9,
    radiusChip: 7,
    radiusCard: 14,
    radiusModal: 16,
    rowTreatment: "card",
    rowGap: 0,
    navActive: "fill-glow",
    background: "aurora",
    glow: 0.62,
    motionScale: 1.3,
  },

  /**
   * Quiet — clean, flat, minimal.
   * Opaque surfaces, no blur, reduced radius, subtle shadows.
   */
  quiet: {
    material: "flat",
    glassBlur: 0,
    saturation: 1.0,
    fillAlpha: 1.0,
    sheen: 0,
    tintAlpha: 0,
    elevation: "none",
    shadowCard: "none",
    shadowFloat: "e1",
    radiusControl: 7,
    radiusChip: 6,
    radiusCard: 10,
    radiusModal: 12,
    rowTreatment: "line",
    rowGap: 0,
    navActive: "tint",
    background: "flat",
    glow: 0,
    motionScale: 1.0,
  },

  /**
   * Vapor — refined glass with stronger blur and tint wash.
   * Higher saturation, accent tint, inset rows, glass-ring nav.
   */
  vapor: {
    material: "glass",
    glassBlur: 34,
    saturation: 1.7,
    fillAlpha: 0.5,
    sheen: 0.16,
    tintAlpha: 0.14,
    elevation: "glass-float",
    shadowCard: "none",
    shadowFloat: "e3",
    radiusControl: 12,
    radiusChip: 10,
    radiusCard: 16,
    radiusModal: 16,
    rowTreatment: "inset",
    rowGap: 3,
    navActive: "glass-ring",
    background: "tint-blob",
    glow: 0.45,
    motionScale: 1.0,
  },
};
