/**
 * useHistoryFilter — search, FTS, device filter, and sort hook for HistoryView.
 *
 * Extracted from HistoryView.tsx (CopyPaste-g06m.34 refactor).
 * Owns: search, ftsResults, ftsQuery, deviceFilter, sortMode, knownDevices,
 *       knownDeviceIds, filtered.
 *
 * Takes items, ownDeviceId, and sortByDevice pref as inputs so the filter
 * logic stays pure (no direct store reads — the view passes prefs down).
 */
import { useCallback, useEffect, useMemo, useState } from "react";
import { api, type HistoryEntry } from "../../../lib/ipc";
import { fuzzyMatch } from "../../../lib/fuzzy";

export function useHistoryFilter(
  items: HistoryEntry[],
  ownDeviceId: string,
  sortByDevice: boolean,
  setPrefs: (prefs: { sortByDevice: boolean }) => void,
) {
  const [search, setSearch] = useState("");
  const [ftsResults, setFtsResults] = useState<Set<string>>(new Set());
  const [ftsQuery, setFtsQuery] = useState("");
  // "all" | device UUID | "this" — filters the list to a specific origin device.
  const [deviceFilter, setDeviceFilter] = useState<string>("all");
  // "recency" (default daemon order) | "device" (group by origin device, then recency within group)
  // Initialised from the persisted sortByDevice pref (bdac.91 — Android parity).
  const [sortMode, setSortMode] = useState<"recency" | "device">(() =>
    sortByDevice ? "device" : "recency"
  );

  // -------------------------------------------------------------------------
  // FTS effect — debounced daemon full-text search over the entire history
  // -------------------------------------------------------------------------

  useEffect(() => {
    const q = search.trim();
    if (!q) {
      setFtsResults(new Set());
      setFtsQuery("");
      return;
    }
    const timer = setTimeout(async () => {
      try {
        const hits = await api.searchItems(q, 500);
        setFtsResults(new Set(hits.map((h) => h.id)));
        setFtsQuery(q);
      } catch {
        // FTS failure falls back gracefully to client-side filter
      }
    }, 250);
    return () => clearTimeout(timer);
  }, [search]);

  // -------------------------------------------------------------------------
  // Distinct device IDs+names seen in loaded items — drives the filter dropdown.
  // v6ac: replaced knownDeviceIds (Set<string>) with knownDevices (Map<id,name>)
  // so the dropdown shows human-readable names instead of hex UUID prefixes.
  // The name is seeded from origin_device_name on the first item per device;
  // the daemon always emits this field from its devices table.
  // -------------------------------------------------------------------------
  const knownDevices = useMemo(() => {
    const map = new Map<string, string>();
    for (const it of items) {
      if (it.origin_device_id && !map.has(it.origin_device_id)) {
        // Prefer the daemon-emitted name; fall back to the compact UUID prefix.
        map.set(it.origin_device_id, it.origin_device_name ?? it.origin_device_id.slice(0, 8));
      }
    }
    return map;
  }, [items]);
  // Stable array of known device ids (same order as Map insertion = first-seen).
  const knownDeviceIds = useMemo(() => Array.from(knownDevices.keys()), [knownDevices]);

  // -------------------------------------------------------------------------
  // Filtered + sorted list — union of client-side substring match, daemon FTS
  // results, and device filter; sorted by the selected sort mode.
  // -------------------------------------------------------------------------

  const filtered = useMemo(() => {
    const q = search.trim();

    // 1. Text search: SCRH-4 — use fuzzyMatch for subsequence matching + score sorting.
    // FTS daemon hits are included as additional matches (no fuzzy score, treated as
    // exact match with score 0 so they appear after scored fuzzy results).
    let result: HistoryEntry[];
    if (q) {
      // Compute fuzzy scores for all items so we can sort by relevance.
      // Items that match neither fuzzy nor FTS are filtered out.
      const scored: Array<{ entry: HistoryEntry; score: number }> = [];
      for (const it of items) {
        const fuzzyResult = fuzzyMatch(q, it.preview);
        if (fuzzyResult !== null) {
          scored.push({ entry: it, score: fuzzyResult.score });
        } else if (ftsQuery === q && ftsResults.has(it.id)) {
          // FTS-only hit (daemon found it but client fuzzy didn't): include at score 0.
          scored.push({ entry: it, score: 0 });
        }
      }
      // Sort descending by score so the best fuzzy match rises to the top.
      // Stable sort preserves the daemon's recency order within equal-score groups.
      scored.sort((a, b) => b.score - a.score);
      result = scored.map((s) => s.entry);
    } else {
      result = items;
    }

    // 2. Device filter
    if (deviceFilter !== "all") {
      result = result.filter((it) => (it.origin_device_id ?? "") === deviceFilter);
    }

    // 3. Sort mode: "device" groups by origin_device_id (own device first, then
    //    alphabetical by id), preserving the daemon's recency order within each group.
    // When a search is active the fuzzy-score order takes precedence; the device
    // grouping is skipped to avoid discarding the relevance ranking.
    if (sortMode === "device" && !q) {
      // Stable sort: JS Array.sort is stable in all modern engines.
      result = [...result].sort((a, b) => {
        const aId = a.origin_device_id ?? "";
        const bId = b.origin_device_id ?? "";
        if (aId === bId) return 0;
        // Own device always sorts first.
        if (ownDeviceId && aId === ownDeviceId) return -1;
        if (ownDeviceId && bId === ownDeviceId) return 1;
        return aId.localeCompare(bId);
      });
    }

    return result;
  }, [items, search, ftsResults, ftsQuery, deviceFilter, sortMode, ownDeviceId]);

  // Build human-readable label for a device id in the filter dropdown.
  // v6ac: uses knownDevices map (id→name) so the dropdown shows names, not hex IDs.
  const deviceOptionLabel = useCallback(
    (id: string): string => {
      if (id === "all") return "All devices";
      if (ownDeviceId && id === ownDeviceId) return "This device";
      // Prefer the name we collected from origin_device_name; fall back to UUID prefix.
      return knownDevices.get(id) ?? id.slice(0, 8);
    },
    [ownDeviceId, knownDevices]
  );

  const toggleSortMode = useCallback(() => {
    const next = sortMode === "recency" ? "device" : "recency";
    setSortMode(next);
    // Persist the choice so Settings > Display > History list > "Group by device" stays in sync.
    setPrefs({ sortByDevice: next === "device" });
  }, [sortMode, setPrefs]);

  return {
    search,
    setSearch,
    ftsResults,
    ftsQuery,
    deviceFilter,
    setDeviceFilter,
    sortMode,
    toggleSortMode,
    knownDevices,
    knownDeviceIds,
    filtered,
    deviceOptionLabel,
  };
}
