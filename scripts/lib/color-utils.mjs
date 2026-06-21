/**
 * color-utils.mjs — Shared color parsing and formatting helpers.
 *
 * No I/O, no side effects. All inputs/outputs are plain values.
 *
 * Exported:
 *   hexToRgb(hex)          — CSS "#RRGGBB" / "#RGB" → [r,g,b] or null
 *   ktHexToRgb(hex)        — Kotlin "0xFFRRGGBB" / "0xRRGGBB" → [r,g,b] or null
 *   tripletToRgb(s)        — "R G B" or "R, G, B" space/comma-separated → [r,g,b] or null
 *   parseRgba(s)           — "rgba(r,g,b,a)" / "rgb(r,g,b)" → { rgb, alpha } or null
 *   fmtRgb(rgb)            — [r,g,b] → "#RRGGBB" (uppercase), or "(null)"
 *   withinTolerance(a,b,t) — true if every channel of a and b differ by ≤ t
 */

/** Parse a CSS hex colour (#RRGGBB or #RGB) to [r,g,b]. */
export function hexToRgb(hex) {
  hex = hex.replace(/^#/, "");
  if (hex.length === 3) hex = hex.split("").map((c) => c + c).join("");
  if (hex.length !== 6) return null;
  const n = parseInt(hex, 16);
  return [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

/** Parse Android 0xFFRRGGBB or 0xRRGGBB hex to [r,g,b]. */
export function ktHexToRgb(hex) {
  // strip 0x prefix and optional alpha byte (8-digit = AARRGGBB)
  hex = hex.replace(/^0[xX]/, "");
  if (hex.length === 8) hex = hex.slice(2); // drop AA
  if (hex.length !== 6) return null;
  const n = parseInt(hex, 16);
  return [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

/** Parse "R G B" or "R, G, B" space/comma-separated triplet to [r,g,b]. */
export function tripletToRgb(s) {
  const parts = s.trim().split(/[\s,]+/);
  if (parts.length !== 3) return null;
  return parts.map(Number);
}

/**
 * Parse "rgba(r,g,b,a)" or "rgba(r, g, b, a)" to { rgb:[r,g,b], alpha:a }.
 * Also handles "rgb(r,g,b)" (alpha=1).
 */
export function parseRgba(s) {
  s = s.trim();
  const rgbaM = s.match(/rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)(?:\s*,\s*([\d.]+))?\s*\)/);
  if (!rgbaM) return null;
  return {
    rgb: [parseInt(rgbaM[1]), parseInt(rgbaM[2]), parseInt(rgbaM[3])],
    alpha: rgbaM[4] !== undefined ? parseFloat(rgbaM[4]) : 1,
  };
}

/** Format [r,g,b] as #RRGGBB (uppercase). Returns "(null)" for null input. */
export function fmtRgb(rgb) {
  if (!rgb) return "(null)";
  return (
    "#" +
    rgb
      .map((c) => c.toString(16).padStart(2, "0").toUpperCase())
      .join("")
  );
}

/**
 * Check if two [r,g,b] triplets are within tolerance per channel.
 * Returns false if either value is null/falsy.
 */
export function withinTolerance(a, b, tol) {
  if (!a || !b) return false;
  return a.every((v, i) => Math.abs(v - b[i]) <= tol);
}
