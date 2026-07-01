import type { ViewId } from "../store";

// ---------------------------------------------------------------------------
// resolveView — defensive `view` narrowing (task 6.3, design.md Decision 6/B3).
//
// The store already types `view: ViewId`, so this is NOT a TypeScript
// narrowing concern — it is a runtime guard for input that could disagree
// with that type at the boundary: a `?view=` URL param, a future call site, a
// hot-reload edge case, or the dev-only `"gallery"` value leaking into a
// production build. Any value that is not one of the five production view
// ids resolves to `"history"`.
//
// This is explicitly NOT persisted-state recovery: `view` lives only in the
// in-memory Zustand store (store.ts has no `persist`/`partialize` of `view` —
// only `UIPrefs` is written to localStorage), so there is no "downgrade
// leaves gallery persisted" case to recover from. Do not add persistence here
// to manufacture one.
// ---------------------------------------------------------------------------

const PRODUCTION_VIEW_IDS: ReadonlySet<string> = new Set<ViewId>([
  "history",
  "devices",
  "settings",
  "about",
  "logs",
]);

/**
 * Narrow an arbitrary string (in-memory store state or URL-derived input) to
 * a known production {@link ViewId}. Anything unrecognized — including the
 * dev-only `"gallery"` value — falls back to `"history"`.
 */
export function resolveView(rawView: string): ViewId {
  return PRODUCTION_VIEW_IDS.has(rawView) ? (rawView as ViewId) : "history";
}
