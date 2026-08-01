/** A WebView has one product surface. The Quick Paste window must never mount
 * the full application shell just because it shares the same Vite entry file. */
export const QUICK_PASTE_SURFACE = "quick-paste";

export function isQuickPasteSurface(search: string): boolean {
  return new URLSearchParams(search).get("surface") === QUICK_PASTE_SURFACE;
}
