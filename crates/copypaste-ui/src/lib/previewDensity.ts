export const MIN_PREVIEW_LINES = 3;
export const MAX_PREVIEW_LINES = 3;
export const DEFAULT_PREVIEW_LINES = 3;

export function previewLineCount(previewLines: number): number {
  return Math.min(
    MAX_PREVIEW_LINES,
    Math.max(MIN_PREVIEW_LINES, Math.round(previewLines)),
  );
}
