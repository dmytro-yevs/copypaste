/**
 * liquid-tokens.ts — Liquid Glass palette/token registry (CopyPaste-52mz)
 *
 * Single source of truth for:
 *   • All 10 palette definitions (exactly as in the styleguide palettes object)
 *   • Contrast profiles (dark/light × soft/balanced/high)
 *   • Density and motion profile constants
 *   • Re-exports used by tests and (optionally) runtime palette switching
 *
 * CSS is the authoritative runtime layer; this module gives the TS side
 * parity so tests can assert exact values without parsing CSS.
 */

// ── Palette key type ──────────────────────────────────────────────────────────

export type PaletteKey =
  | "liquid-blue"
  | "graphite-mist"
  | "deep-sky"
  | "nordic-cyan"
  | "cloud-silver"
  | "frost-blue"
  | "porcelain"
  | "pearl-grey"
  | "aurora-violet"
  | "amber-night";

export const PALETTE_KEYS: PaletteKey[] = [
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

// ── Palette definition shape ──────────────────────────────────────────────────

export interface PaletteDef {
  name: string;
  scheme: "dark" | "light";
  bg0: string;
  bg1: string;
  bg2: string;
  glowA: string;
  glowB: string;
  /** CSS RGB triplet e.g. "28, 31, 42" */
  surfaceRgb: string;
  surfaceStrongRgb: string;
  accent: string;
  accent2: string;
  accent3: string;
  onAccent: string;
  success: string;
  warning: string;
  danger: string;
  // Glass material
  glassOpacity: number;
  glassBlur: string;
  glassSaturation: number;
  glowStrength: number;
  // Motion (cinematic defaults)
  speed: number;
  motionOpacity: number;
}

// ── All 10 palettes (exact values from styleguide lines 2516–2597) ────────────

export const PALETTES: Record<PaletteKey, PaletteDef> = {
  "liquid-blue": {
    name: "Liquid Blue", scheme: "dark",
    bg0: "#061123", bg1: "#0b1f47", bg2: "#12152d",
    glowA: "#4d8dff", glowB: "#9e7bff",
    surfaceRgb: "22, 30, 54", surfaceStrongRgb: "30, 42, 76",
    accent: "#4d8dff", accent2: "#7fc7ff", accent3: "#9e7bff", onAccent: "#ffffff",
    success: "#67df81", warning: "#ffbd45", danger: "#ff6b78",
    glassOpacity: 0.64, glassBlur: "28px", glassSaturation: 1.45, glowStrength: 0.62,
    speed: 0.72, motionOpacity: 1,
  },
  "graphite-mist": {
    name: "Graphite Mist", scheme: "dark",
    bg0: "#07090f", bg1: "#141924", bg2: "#202330",
    glowA: "#7f8da3", glowB: "#5a6a83",
    surfaceRgb: "28, 31, 42", surfaceStrongRgb: "42, 46, 60",
    accent: "#9db7df", accent2: "#d5e2f7", accent3: "#7c8ca6", onAccent: "#ffffff",
    success: "#7be0b1", warning: "#ffcc6a", danger: "#ff7f8c",
    glassOpacity: 0.64, glassBlur: "28px", glassSaturation: 1.45, glowStrength: 0.62,
    speed: 0.72, motionOpacity: 1,
  },
  "deep-sky": {
    name: "Deep Sky", scheme: "dark",
    bg0: "#021222", bg1: "#073766", bg2: "#101b31",
    glowA: "#1fa4ff", glowB: "#55e6ff",
    surfaceRgb: "16, 32, 56", surfaceStrongRgb: "24, 48, 82",
    accent: "#1f9cff", accent2: "#8cddff", accent3: "#4d74ff", onAccent: "#ffffff",
    success: "#6ee79e", warning: "#ffc75d", danger: "#ff6b78",
    glassOpacity: 0.64, glassBlur: "28px", glassSaturation: 1.45, glowStrength: 0.62,
    speed: 0.72, motionOpacity: 1,
  },
  "nordic-cyan": {
    name: "Nordic Cyan", scheme: "dark",
    bg0: "#031216", bg1: "#06333b", bg2: "#0b1d2b",
    glowA: "#24d6b5", glowB: "#478cff",
    surfaceRgb: "15, 37, 49", surfaceStrongRgb: "21, 55, 70",
    accent: "#25d5b4", accent2: "#8af5e4", accent3: "#5a9dff", onAccent: "#ffffff",
    success: "#75ef9f", warning: "#ffd166", danger: "#ff6b6b",
    glassOpacity: 0.64, glassBlur: "28px", glassSaturation: 1.45, glowStrength: 0.62,
    speed: 0.72, motionOpacity: 1,
  },
  "cloud-silver": {
    name: "Cloud Silver", scheme: "light",
    bg0: "#edf2f8", bg1: "#dce6f3", bg2: "#f8fbff",
    glowA: "#b9c7d9", glowB: "#dce7f7",
    surfaceRgb: "255, 255, 255", surfaceStrongRgb: "245, 248, 252",
    accent: "#5b8def", accent2: "#2f74e8", accent3: "#9cb3d7", onAccent: "#ffffff",
    success: "#158f48", warning: "#a86700", danger: "#d93b4a",
    glassOpacity: 0.64, glassBlur: "28px", glassSaturation: 1.45, glowStrength: 0.62,
    speed: 0.72, motionOpacity: 1,
  },
  "frost-blue": {
    name: "Frost Blue", scheme: "light",
    bg0: "#edf7ff", bg1: "#d9ecff", bg2: "#f8fcff",
    glowA: "#91c9ff", glowB: "#c7e6ff",
    surfaceRgb: "255, 255, 255", surfaceStrongRgb: "238, 247, 255",
    accent: "#2777ff", accent2: "#005fe3", accent3: "#72b8ff", onAccent: "#ffffff",
    success: "#108747", warning: "#9a6200", danger: "#ca3446",
    glassOpacity: 0.64, glassBlur: "28px", glassSaturation: 1.45, glowStrength: 0.62,
    speed: 0.72, motionOpacity: 1,
  },
  "porcelain": {
    name: "Porcelain", scheme: "light",
    bg0: "#f3f6fa", bg1: "#e4ebf4", bg2: "#fbfdff",
    glowA: "#a9c9ef", glowB: "#d2e5fb",
    surfaceRgb: "255, 255, 255", surfaceStrongRgb: "246, 250, 255",
    accent: "#3c7dd9", accent2: "#1e5fb9", accent3: "#8fb8e8", onAccent: "#ffffff",
    success: "#16874c", warning: "#9b6500", danger: "#ca3446",
    glassOpacity: 0.64, glassBlur: "28px", glassSaturation: 1.45, glowStrength: 0.62,
    speed: 0.72, motionOpacity: 1,
  },
  "pearl-grey": {
    name: "Pearl Grey", scheme: "light",
    bg0: "#f1f1f2", bg1: "#dedfe3", bg2: "#fafafa",
    glowA: "#aeb4bd", glowB: "#d4d6dc",
    surfaceRgb: "255, 255, 255", surfaceStrongRgb: "243, 244, 247",
    accent: "#58677f", accent2: "#34465f", accent3: "#9ba6b7", onAccent: "#ffffff",
    success: "#1c8f50", warning: "#9b6400", danger: "#c93445",
    glassOpacity: 0.64, glassBlur: "28px", glassSaturation: 1.45, glowStrength: 0.62,
    speed: 0.72, motionOpacity: 1,
  },
  "aurora-violet": {
    name: "Aurora Violet", scheme: "dark",
    bg0: "#11071f", bg1: "#28114d", bg2: "#14172d",
    glowA: "#9a7cff", glowB: "#ff7ad9",
    surfaceRgb: "29, 24, 54", surfaceStrongRgb: "48, 36, 82",
    accent: "#9a7cff", accent2: "#d4b6ff", accent3: "#ff7ad9", onAccent: "#ffffff",
    success: "#6ee7b7", warning: "#ffc35a", danger: "#ff6f91",
    glassOpacity: 0.64, glassBlur: "28px", glassSaturation: 1.45, glowStrength: 0.62,
    speed: 0.72, motionOpacity: 1,
  },
  "amber-night": {
    name: "Amber Night", scheme: "dark",
    bg0: "#171008", bg1: "#3a220a", bg2: "#1c1a22",
    glowA: "#ff9f1c", glowB: "#6ca0ff",
    surfaceRgb: "35, 29, 24", surfaceStrongRgb: "58, 43, 30",
    accent: "#ffad33", accent2: "#ffd28a", accent3: "#6ca0ff", onAccent: "#ffffff",
    success: "#82e070", warning: "#ffbf47", danger: "#ff6b68",
    glassOpacity: 0.64, glassBlur: "28px", glassSaturation: 1.45, glowStrength: 0.62,
    speed: 0.72, motionOpacity: 1,
  },
};

// ── Convenience: Graphite Mist defaults (the active default palette) ──────────

export const LIQUID_DEFAULTS = PALETTES["graphite-mist"];

// ── Scheme map (palette key → "dark"|"light") ─────────────────────────────────

export const PALETTE_SCHEMES: Record<PaletteKey, "dark" | "light"> = Object.fromEntries(
  PALETTE_KEYS.map((k) => [k, PALETTES[k].scheme]),
) as Record<PaletteKey, "dark" | "light">;

// ── Contrast profiles (styleguide lines 2599–2628) ────────────────────────────

export type ContrastLevel = "soft" | "balanced" | "high";

export interface ContrastProfile {
  text: string;
  textSoft: string;
  textMuted: string;
  textDisabled: string;
  icon: string;
  iconMuted: string;
  line: string;
  lineStrong: string;
  border: string;
  borderStrong: string;
}

export const CONTRAST_PROFILES: Record<"dark" | "light", Record<ContrastLevel, ContrastProfile>> = {
  dark: {
    soft: {
      text: "rgba(248,250,255,.90)", textSoft: "rgba(229,236,255,.68)",
      textMuted: "rgba(217,225,244,.46)", textDisabled: "rgba(217,225,244,.30)",
      icon: "rgba(245,249,255,.84)", iconMuted: "rgba(220,229,248,.54)",
      line: "rgba(255,255,255,.10)", lineStrong: "rgba(255,255,255,.18)",
      border: "rgba(255,255,255,.12)", borderStrong: "rgba(255,255,255,.20)",
    },
    balanced: {
      text: "rgba(248,250,255,.96)", textSoft: "rgba(229,236,255,.78)",
      textMuted: "rgba(217,225,244,.58)", textDisabled: "rgba(217,225,244,.38)",
      icon: "rgba(245,249,255,.92)", iconMuted: "rgba(220,229,248,.66)",
      line: "rgba(255,255,255,.14)", lineStrong: "rgba(255,255,255,.24)",
      border: "rgba(255,255,255,.16)", borderStrong: "rgba(255,255,255,.26)",
    },
    high: {
      text: "rgba(255,255,255,.99)", textSoft: "rgba(241,246,255,.88)",
      textMuted: "rgba(226,235,250,.70)", textDisabled: "rgba(226,235,250,.48)",
      icon: "rgba(255,255,255,.98)", iconMuted: "rgba(236,244,255,.80)",
      line: "rgba(255,255,255,.19)", lineStrong: "rgba(255,255,255,.32)",
      border: "rgba(255,255,255,.23)", borderStrong: "rgba(255,255,255,.36)",
    },
  },
  light: {
    soft: {
      text: "rgba(18,25,38,.84)", textSoft: "rgba(41,52,70,.66)",
      textMuted: "rgba(61,76,99,.48)", textDisabled: "rgba(73,88,111,.32)",
      icon: "rgba(18,25,38,.80)", iconMuted: "rgba(48,63,86,.56)",
      line: "rgba(25,35,52,.10)", lineStrong: "rgba(25,35,52,.18)",
      border: "rgba(25,35,52,.12)", borderStrong: "rgba(25,35,52,.20)",
    },
    balanced: {
      text: "rgba(13,20,32,.92)", textSoft: "rgba(35,47,67,.76)",
      textMuted: "rgba(52,67,90,.58)", textDisabled: "rgba(70,86,111,.40)",
      icon: "rgba(13,20,32,.88)", iconMuted: "rgba(42,56,80,.66)",
      line: "rgba(20,30,48,.14)", lineStrong: "rgba(20,30,48,.23)",
      border: "rgba(20,30,48,.16)", borderStrong: "rgba(20,30,48,.26)",
    },
    high: {
      text: "rgba(8,13,22,.98)", textSoft: "rgba(23,34,52,.86)",
      textMuted: "rgba(36,50,73,.70)", textDisabled: "rgba(49,65,90,.52)",
      icon: "rgba(8,13,22,.96)", iconMuted: "rgba(27,42,66,.80)",
      line: "rgba(12,20,35,.19)", lineStrong: "rgba(12,20,35,.31)",
      border: "rgba(12,20,35,.22)", borderStrong: "rgba(12,20,35,.36)",
    },
  },
};
