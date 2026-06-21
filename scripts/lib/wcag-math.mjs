/**
 * wcag-math.mjs — Pure WCAG 2.1 contrast-ratio helpers.
 *
 * No I/O, no side effects. All inputs/outputs are plain numbers or [r,g,b] arrays.
 *
 * Exported:
 *   toLinear(c)            — sRGB channel (0–255) → linear light
 *   luminance([r,g,b])     — WCAG relative luminance
 *   contrast(fg, bg)       — WCAG contrast ratio (both [r,g,b], 0–255)
 *   composite(fg,alpha,bg) — Porter-Duff src-over: fg+alpha over opaque bg → [r,g,b]
 */

/** Convert an sRGB channel value (0–255) to linear light. */
export function toLinear(c) {
  const s = c / 255;
  return s <= 0.04045 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
}

/** Compute WCAG relative luminance for an [r,g,b] triplet (0–255 each). */
export function luminance([r, g, b]) {
  return 0.2126 * toLinear(r) + 0.7152 * toLinear(g) + 0.0722 * toLinear(b);
}

/** Compute WCAG contrast ratio for two [r,g,b] colors. */
export function contrast(fg, bg) {
  const L1 = Math.max(luminance(fg), luminance(bg));
  const L2 = Math.min(luminance(fg), luminance(bg));
  return (L1 + 0.05) / (L2 + 0.05);
}

/**
 * Composite a foreground color with alpha over an opaque background.
 * fg: [r, g, b] (0–255), alpha: 0–1, bg: [r, g, b] (0–255)
 * Returns opaque [r, g, b].
 */
export function composite(fg, alpha, bg) {
  return fg.map((c, i) => Math.round(c * alpha + bg[i] * (1 - alpha)));
}
