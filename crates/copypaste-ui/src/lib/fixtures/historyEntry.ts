// ---------------------------------------------------------------------------
// src/lib/fixtures/historyEntry.ts — typed HistoryEntry fixture factory.
//
// DEV-only. See index.ts for the import-boundary rule.
// ---------------------------------------------------------------------------

import type { HistoryEntry } from "../ipc";
import { FIXTURE_OWN_DEVICE_ID } from "./ids";
import { mins } from "./time";

/**
 * Typed factory for a clipboard {@link HistoryEntry}, with per-field override
 * support (design.md Decision 7/G3). Defaults describe a plain, non-sensitive,
 * local text item captured "just now" — pass `over` to build any other
 * documented row shape (a different kind, sensitive, from a peer device, …).
 *
 * The default `id` is a fixed constant, NOT a counter — callers that render
 * more than one entry at once (a list, a gallery section) MUST override `id`
 * per call so React keys stay stable across re-renders. mockIpc.ts's fixed
 * per-item ids (`"item-001"`, …) are the reference example.
 */
export function makeHistoryEntry(over: Partial<HistoryEntry> = {}): HistoryEntry {
  return {
    id: "fixture-history-entry",
    content_type: "text",
    preview: "Sample clipboard text",
    is_sensitive: false,
    wall_time: mins(1),
    pinned: false,
    kind: "TEXT",
    origin_device_id: FIXTURE_OWN_DEVICE_ID,
    origin_device_name: null,
    app_bundle_id: null,
    ...over,
  };
}
