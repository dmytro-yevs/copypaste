package com.copypaste.android

import androidx.activity.compose.BackHandler
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import kotlinx.coroutines.delay

// ─────────────────────────────────────────────────────────────────────────────
// CopyPaste-vp63.37 — HistoryScreenState: pure item-pipeline derivations
// extracted from HistoryScreen's inline `remember` blocks so the sort/filter
// logic is unit-testable without Compose (see HistoryScreenStateTest), PLUS
// (this pass) the hoisted holder for the screen's search/filter/selection/
// reorder/preview/confirm state fields — replaces the 12 separate
// `var x by rememberSaveable {...}` declarations that used to live directly
// inside HistoryScreen (now in HistoryScreen.kt). Each field below uses the
// exact same saver/initial-value as the original inline declaration, so
// rotation-survival behaviour is unchanged.
//
// The derived item pipelines (sortedItems/filteredItems/deviceFilteredItems)
// and their LaunchedEffects, plus the cross-cutting reorder/selection/preview
// effects, are ALSO hoisted here (`rememberHistoryItemPipelines` /
// `HistoryScreenEffects` further below) — moved verbatim out of HistoryScreen,
// which stays focused on Scaffold/topBar orchestration.
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Hoisted HistoryScreen state — the "nouns" (search/filter/selection/reorder/
 * preview/confirm-dialog fields) that used to be inline `rememberSaveable`
 * vars inside the `HistoryScreen` composable. Construct via
 * [rememberHistoryScreenState]; every property is backed by the same
 * MutableState instance the original inline declaration used, so this is a
 * pure hoist — no behaviour change.
 */
@Stable
class HistoryScreenState internal constructor(
    searchQueryState: MutableState<String>,
    searchExpandedState: MutableState<Boolean>,
    recentSearchesState: MutableState<List<String>>,
    deviceFilterState: MutableState<String>,
    selectionModeState: MutableState<Boolean>,
    selectedIdsState: MutableState<Set<String>>,
    pendingConfirmState: MutableState<ConfirmAction?>,
    pendingDeleteIdState: MutableState<String?>,
    overflowExpandedState: MutableState<Boolean>,
    reorderModeState: MutableState<Boolean>,
    sortByDeviceState: MutableState<Boolean>,
    previewItemIdState: MutableState<String?>,
    previewPhaseState: MutableState<PreviewPhase>,
    isDegradedState: MutableState<Boolean>,
) {
    var searchQuery by searchQueryState
    // HW-A8: icon-toggle search bar — expanded state + last-5 recent queries.
    var searchExpanded by searchExpandedState
    // Recent searches are PERSISTED in Settings (SharedPreferences), not just
    // across rotation — so they survive process death. Seeded once from settings.
    var recentSearches by recentSearchesState

    // "all" = no filter; any other value = UUID of the origin device to show.
    var deviceFilter by deviceFilterState

    var selectionMode by selectionModeState
    var selectedIds by selectedIdsState

    var pendingConfirm by pendingConfirmState
    // CopyPaste-2ifa: id of the single item whose delete is waiting for confirmation.
    var pendingDeleteId by pendingDeleteIdState
    var overflowExpanded by overflowExpandedState

    // Reorder mode (pinned items only).
    var reorderMode by reorderModeState

    // CopyPaste-un29: sort / group by device — persisted in Settings.
    var sortByDevice by sortByDeviceState

    // Long-press peek preview state.
    var previewItemId by previewItemIdState
    var previewPhase by previewPhaseState

    /**
     * android-history 5.3 — NEW persistent error/degraded presentation state
     * (S5-owned plumbing, no repository/IPC change). `ClipboardViewModel.errors`
     * is a one-shot/transient LiveData (HistoryScreen clears it right after
     * showing the toast), so it cannot alone satisfy spec.md's "a persistent
     * error/degraded state is shown in the list surface itself ... not
     * communicated solely via a transient toast". This flag is set the moment
     * an error is observed and cleared only once a load subsequently succeeds
     * with data (see [clearsDegradedState]) — NOT `rememberSaveable`, since a
     * fresh `loadItems()` always fires on screen entry/recreation and will
     * re-derive the true state from scratch.
     */
    var isDegraded by isDegradedState
}

/**
 * Builds and remembers the [HistoryScreenState] holder for `HistoryScreen`.
 * Every `rememberSaveable`/`remember` call below is byte-identical to the
 * inline declaration it replaces (same initial value, same custom Saver),
 * so rotation-survival semantics are unchanged.
 */
@Composable
fun rememberHistoryScreenState(settings: Settings): HistoryScreenState {
    val searchQueryState = rememberSaveable { mutableStateOf("") }
    val searchExpandedState = rememberSaveable { mutableStateOf(false) }
    val recentSearchesState = remember { mutableStateOf(settings.recentSearches) }
    val deviceFilterState = rememberSaveable { mutableStateOf("all") }
    val selectionModeState = rememberSaveable { mutableStateOf(false) }
    val selectedIdsState = rememberSaveable(
        stateSaver = listSaver(
            save    = { it.toList() },
            restore = { it.toSet() },
        )
    ) { mutableStateOf(setOf<String>()) }
    val pendingConfirmState = rememberSaveable(
        stateSaver = androidx.compose.runtime.saveable.Saver(
            save    = { it?.ordinal },
            restore = { ord -> ConfirmAction.entries.getOrNull(ord) },
        )
    ) { mutableStateOf<ConfirmAction?>(null) }
    val pendingDeleteIdState = rememberSaveable { mutableStateOf<String?>(null) }
    val overflowExpandedState = rememberSaveable { mutableStateOf(false) }
    val reorderModeState = rememberSaveable { mutableStateOf(false) }
    val sortByDeviceState = rememberSaveable { mutableStateOf(settings.sortByDevice) }
    val previewItemIdState = rememberSaveable { mutableStateOf<String?>(null) }
    val previewPhaseState = rememberSaveable(
        stateSaver = androidx.compose.runtime.saveable.Saver(
            save    = { phase: PreviewPhase ->
                when (phase) {
                    PreviewPhase.Idle    -> 0
                    PreviewPhase.Peeking -> 1
                    PreviewPhase.Pinned  -> 2
                }
            },
            restore = { ord: Int ->
                when (ord) {
                    1    -> PreviewPhase.Peeking
                    2    -> PreviewPhase.Pinned
                    else -> PreviewPhase.Idle
                }
            },
        )
    ) { mutableStateOf<PreviewPhase>(PreviewPhase.Idle) }
    val isDegradedState = remember { mutableStateOf(false) }

    return remember {
        HistoryScreenState(
            searchQueryState = searchQueryState,
            searchExpandedState = searchExpandedState,
            recentSearchesState = recentSearchesState,
            deviceFilterState = deviceFilterState,
            selectionModeState = selectionModeState,
            selectedIdsState = selectedIdsState,
            pendingConfirmState = pendingConfirmState,
            pendingDeleteIdState = pendingDeleteIdState,
            overflowExpandedState = overflowExpandedState,
            reorderModeState = reorderModeState,
            sortByDeviceState = sortByDeviceState,
            previewItemIdState = previewItemIdState,
            previewPhaseState = previewPhaseState,
            isDegradedState = isDegradedState,
        )
    }
}

/**
 * android-history 5.3 — the pure decision behind [HistoryScreenState.isDegraded]'s
 * recovery: a load cycle that finishes (`loading` false) WITH data
 * (`hasItems` true) proves the data source is reachable again. A load that
 * finishes with zero items after an error stays degraded (a real "cleared
 * history" and a still-failing daemon connection are indistinguishable from
 * `items`/`loading` alone) until the user explicitly retries — a documented,
 * conservative limitation (CopyPaste-myh8.5 bd notes) rather than a richer
 * explicit backend "degraded" signal, which would require a ClipboardViewModel/
 * IPC change out of this slice's scope.
 */
internal fun clearsDegradedState(loading: Boolean, hasItems: Boolean): Boolean = !loading && hasItems

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

// ─────────────────────────────────────────────────────────────────────────────
// Item pipelines — hoisted out of HistoryScreen.kt to keep that file focused
// on Scaffold/topBar orchestration. Moved verbatim; only the surrounding
// `remember`/`LaunchedEffect` scaffolding changed (now inside this composable
// instead of inline in HistoryScreen), values are unchanged.
// ─────────────────────────────────────────────────────────────────────────────

/**
 * The derived item lists + device identity HistoryScreen needs: sort (pinned-
 * first, optionally grouped by device), search filter (snippet + debounced
 * full-content), and device filter — in that pipeline order, exactly as the
 * original inline `remember` chain in HistoryScreen computed them.
 */
@Stable
class HistoryItemPipelines internal constructor(
    val ownDeviceId: String,
    val pairedPeers: List<PairedPeer>,
    val sortedItems: List<ClipboardItem>,
    val filteredItems: List<ClipboardItem>,
    val deviceFilteredItems: List<ClipboardItem>,
    val originDeviceIds: Set<String>,
)

/**
 * Builds [HistoryItemPipelines] for the current [items]/[state]. Mirrors the
 * exact `remember`/`LaunchedEffect` chain that used to live inline in
 * HistoryScreen (sortedItems -> full-content search effect -> filteredItems
 * -> originDeviceIds -> device-filter auto-reset effect -> deviceFilteredItems).
 */
@Composable
fun rememberHistoryItemPipelines(
    items: List<ClipboardItem>,
    state: HistoryScreenState,
    settings: Settings,
    repository: ClipboardRepository,
): HistoryItemPipelines {
    // ── Device identity — needed for sort-by-device and device-filter ───────────
    // Defined before sortedItems because sortByDevice sort references them.
    val ownDeviceId = remember { settings.deviceId }
    val pairedPeers = remember { settings.pairedPeers }

    // Sort: pinned first (by user-defined pinnedSortIndex), then unpinned by recency.
    // CopyPaste-un29: when sortByDevice is true, group unpinned items by origin device
    // (own device first, then peers alphabetically by display name, null-origin last),
    // then by recency within each device group — mirrors macOS HistoryView device sort.
    // Pinned items always remain at the top in user-defined order regardless of the sort.
    val sortedItems = remember(items, state.sortByDevice, ownDeviceId, pairedPeers) {
        sortHistoryItems(items, state.sortByDevice, ownDeviceId, pairedPeers)
    }

    // ── AB-11: full-content search ───────────────────────────────────────────
    // The snippet-only filter missed any match past the 140-char preview. We now
    // ALSO match the full decrypted text. To stay responsive we (a) show instant
    // snippet matches synchronously, and (b) compute full-content matches in the
    // background (debounced) and union them in once ready.
    var fullMatchIds by remember { mutableStateOf<Set<String>>(emptySet()) }
    var fullMatchQuery by remember { mutableStateOf("") }

    // F: key only on searchQuery (not sortedItems) so the effect does not re-fire
    // on every list re-emit when the query is empty. When query is non-empty we
    // also hash the id list so a new item appearing while searching still
    // triggers a fresh full-content scan.
    val idListHash = remember(sortedItems) { sortedItems.map { it.id }.hashCode() }
    LaunchedEffect(state.searchQuery, if (state.searchQuery.isBlank()) 0 else idListHash) {
        val q = state.searchQuery.trim()
        if (q.isEmpty()) {
            fullMatchIds = emptySet()
            fullMatchQuery = ""
            return@LaunchedEffect
        }
        // Debounce: wait out rapid keystrokes before the (decrypting) full scan.
        delay(250)
        val key = settings.encryptionKey
        val ids = sortedItems.map { it.id }
        fullMatchIds = repository.searchIds(ids, q, key)
        fullMatchQuery = q
    }

    // Filter: snippet match (instant) ∪ full-content match (async, debounced).
    val filteredItems = remember(sortedItems, state.searchQuery, fullMatchIds, fullMatchQuery) {
        filterHistoryItemsBySearch(sortedItems, state.searchQuery, fullMatchIds, fullMatchQuery)
    }

    // ── Device filter (parity with macOS) ────────────────────────────────────
    // Collect distinct origin device ids from the FULL sorted list (not search-
    // filtered) so the filter chips are stable while typing. Show the chips only
    // when more than one device is present — mirrors macOS HistoryView.
    val originDeviceIds = remember(sortedItems) { distinctOriginDeviceIds(sortedItems) }

    // Auto-reset device filter when the selected device disappears from the list
    // (e.g. all items from that device were deleted).
    LaunchedEffect(originDeviceIds, state.deviceFilter) {
        if (state.deviceFilter != "all" && state.deviceFilter !in originDeviceIds) {
            state.deviceFilter = "all"
        }
    }

    // Apply device filter on top of search filter.
    val deviceFilteredItems = remember(filteredItems, state.deviceFilter) {
        filterByDevice(filteredItems, state.deviceFilter)
    }

    return remember(sortedItems, filteredItems, deviceFilteredItems, originDeviceIds, ownDeviceId, pairedPeers) {
        HistoryItemPipelines(
            ownDeviceId = ownDeviceId,
            pairedPeers = pairedPeers,
            sortedItems = sortedItems,
            filteredItems = filteredItems,
            deviceFilteredItems = deviceFilteredItems,
            originDeviceIds = originDeviceIds,
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Cross-cutting screen effects — reorder/selection BackHandlers + the preview/
// selection-collapse and stale-selection-pruning LaunchedEffects. None of
// these read the item pipelines above, so grouping them in one call (in
// either order relative to `rememberHistoryItemPipelines`) preserves the
// original inline behaviour: each LaunchedEffect/BackHandler is registered
// independently in Compose's slot table and none of them depends on another's
// result within the same recomposition.
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Registers the screen-wide BackHandlers + LaunchedEffects that manage
 * reorder mode, the peek-preview auto-dismiss/collapse rules, and pruning
 * `selectedIds` when items disappear from the underlying list. Moved verbatim
 * out of HistoryScreen — see the call site for how [state] feeds the rest of
 * the screen.
 */
@Composable
fun HistoryScreenEffects(items: List<ClipboardItem>, state: HistoryScreenState) {
    BackHandler(enabled = state.reorderMode) { state.reorderMode = false }

    // Auto-dismiss when the previewed item is no longer in the list.
    LaunchedEffect(items, state.previewItemId) {
        val id = state.previewItemId ?: return@LaunchedEffect
        if (items.none { it.id == id }) {
            state.previewItemId = null
            state.previewPhase = PreviewPhase.Idle
        }
    }

    // Entering selection mode collapses any open preview.
    LaunchedEffect(state.selectionMode) {
        if (state.selectionMode && state.previewPhase != PreviewPhase.Idle) {
            state.previewItemId = null
            state.previewPhase = PreviewPhase.Idle
        }
    }

    BackHandler(enabled = state.selectionMode) {
        state.selectionMode = false
        state.selectedIds = emptySet()
    }

    // Entering selection mode exits reorder mode and collapses any open preview
    LaunchedEffect(state.selectionMode) {
        if (state.selectionMode) {
            state.reorderMode = false
            // Collapse preview when selection mode activates
            if (state.previewPhase != PreviewPhase.Idle) {
                state.previewItemId = null
                state.previewPhase = PreviewPhase.Idle
            }
        }
    }

    // Drop selected ids that no longer exist when the underlying list changes
    // (background sync eviction, prune, TTL, remote delete) so the selected
    // count stays accurate. Intersect against the FULL `items` list — not the
    // search-filtered view — so selected-but-hidden items are not wrongly lost.
    LaunchedEffect(items) {
        if (state.selectionMode) {
            val currentIds = items.mapTo(HashSet()) { it.id }
            val pruned = state.selectedIds.intersect(currentIds)
            if (pruned.size != state.selectedIds.size) {
                state.selectedIds = pruned
                if (pruned.isEmpty()) state.selectionMode = false
            }
        }
    }
}
