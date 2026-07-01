package com.copypaste.android

// ─────────────────────────────────────────────────────────────────────────────
// CopyPaste-vp63.37 — HistoryScreenState: pure item-pipeline derivations
// extracted from HistoryScreen's inline `remember` blocks so the sort/filter
// logic is unit-testable without Compose (see HistoryScreenStateTest).
//
// Scope note: this is a first, low-risk slice of the full HistoryScreenState
// holder described in the vp63.37 split sketch (which also covers selection/
// preview/reorder state hoisting, the file picker, and confirm-dialog action
// dispatch). Those pieces stay inline in HistoryActivity.kt for now — this
// file only carries the two PURE list-derivation functions that had zero
// Compose/coroutine dependencies, to avoid an unverifiable, higher-risk
// restructure of the interdependent `remember`/`LaunchedEffect` state in this
// build-blocked (no-gradle) session.
// ─────────────────────────────────────────────────────────────────────────────

/**
 * §5/CopyPaste-un29 — sort items for the history list: pinned first (in
 * user-defined [ClipboardItem.pinnedSortIndex] order), then unpinned by
 * recency. When [sortByDevice] is true, unpinned items are additionally
 * grouped by origin device (own device first, then peers alphabetically by
 * display name, null-origin last) before the recency sort within each group —
 * mirrors macOS HistoryView device sort.
 *
 * De-dupes by [ClipboardItem.id] first: the LazyColumn key is `it.id`, so a
 * duplicate id would throw `IllegalArgumentException` and crash-loop the
 * screen; collapsing duplicates here is a defensive guard against upstream
 * repository drift.
 */
internal fun sortHistoryItems(
    items: List<ClipboardItem>,
    sortByDevice: Boolean,
    ownDeviceId: String,
    pairedPeers: List<PairedPeer>,
): List<ClipboardItem> {
    val deduped = items.distinctBy { it.id }
    return if (sortByDevice) {
        // Group-by-device: pinned section first (user order), then device groups
        // sorted own-device → peer-alphabetical → unknown. Within each device the
        // items are sorted by recency (newest first) for macOS parity.
        deduped.sortedWith(
            compareByDescending<ClipboardItem> { it.pinned }
                .thenBy { if (it.pinned) it.pinnedSortIndex else 0 }
                // Own device first (null originDeviceId treated as own/local).
                .thenByDescending { item ->
                    item.originDeviceId == null || item.originDeviceId == ownDeviceId
                }
                // Peer display name alphabetical for remaining devices.
                .thenBy { item ->
                    item.originDeviceId?.let { id ->
                        deviceDisplayName(id, ownDeviceId, pairedPeers)
                    } ?: ""
                }
                // Recency within each device group.
                .thenByDescending { it.wallTimeMs }
        )
    } else {
        deduped.sortedWith(
            compareByDescending<ClipboardItem> { it.pinned }
                .thenBy { if (it.pinned) it.pinnedSortIndex else 0 }
                .thenByDescending { it.wallTimeMs }
        )
    }
}

/**
 * AB-11 — full-content search filter: snippet match (instant) UNION
 * full-content match (async, debounced). [fullMatchIds] is only trusted when
 * [fullMatchQuery] equals the (trimmed) [query] currently being searched —
 * otherwise the search falls back to the snippet-only match until the async
 * full-content scan catches up with the latest keystrokes.
 *
 * Returns [items] unchanged when [query] is blank.
 */
internal fun filterHistoryItemsBySearch(
    items: List<ClipboardItem>,
    query: String,
    fullMatchIds: Set<String>,
    fullMatchQuery: String,
): List<ClipboardItem> {
    val q = query.trim()
    if (q.isEmpty()) return items
    // Only trust fullMatchIds when it was computed for the CURRENT query;
    // otherwise fall back to the snippet match alone until it catches up.
    val useFull = fullMatchQuery == q
    return items.filter { item ->
        item.snippet.contains(q, ignoreCase = true) ||
            (useFull && item.id in fullMatchIds)
    }
}
