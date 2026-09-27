export const MIN_PREVIEW_LINES = 3;
export const MAX_PREVIEW_LINES = 3;
export const DEFAULT_PREVIEW_LINES = 3;

/** Quick Paste is an action list, so every item reserves a single text line. */
export const QUICK_PASTE_PREVIEW_LINES = 1;

export type PreviewDensitySurface = "history" | "quickPaste";

export function previewLineCount(
  previewLines: number,
  surface: PreviewDensitySurface = "history",
): number {
  if (surface === "quickPaste") return QUICK_PASTE_PREVIEW_LINES;
  return Math.min(
    MAX_PREVIEW_LINES,
    Math.max(MIN_PREVIEW_LINES, Math.round(previewLines)),
  );
}
