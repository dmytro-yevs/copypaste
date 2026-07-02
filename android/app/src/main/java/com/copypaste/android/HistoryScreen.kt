@file:OptIn(ExperimentalFoundationApi::class)

package com.copypaste.android

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.calculateEndPadding
import androidx.compose.foundation.layout.calculateStartPadding
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.livedata.observeAsState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.copypaste.android.ui.GlassToastHost
import com.copypaste.android.ui.GlassToastKind
import com.copypaste.android.ui.GlassToastState
import kotlinx.coroutines.launch

/**
 * History screen — Compose list of clipboard items with macOS parity.
 *
 * Row behaviour:
 *   - Tapping a row copies the item (single-tap = copy, no explicit Copy button)
 *   - Per-row checkbox (always visible) — tapping it enters multi-select mode
 *   - Long-press also enters multi-select mode and selects the tapped row
 *   - In selection mode: bulk action bar replaces the top bar (delete/pin)
 *   - Action buttons on expand: icon-only pin/unpin + delete (no text labels)
 *   - Timestamp always visible in the right gutter (tabular-nums)
 *   - Pinned items shown with a warning-coloured bookmark indicator
 *   - Press-scale (0.98) on rows and action buttons for tactile feel (§8)
 *   - List item mount fade/rise via AnimatedVisibility (§8)
 *
 * CopyPaste-vp63.37: this composable used to live in HistoryActivity.kt
 * together with the Activity shell, HistoryList, and every state
 * declaration. Screen state now lives in [HistoryScreenState]
 * (`rememberHistoryScreenState`), the file picker in
 * `rememberHistoryFilePickerLauncher` (HistoryFilePicker.kt), and the bulk-
 * copy / save-file / open-file / preview-copy action bodies in
 * HistoryItemActions.kt. HistoryActivity.kt is now a thin Activity shell.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HistoryScreen(
    viewModel: ClipboardViewModel = viewModel(),
    modifier: Modifier = Modifier,
    showBackButton: Boolean = true,
    onBack: () -> Unit = {},
    /**
     * §1: paint the canvas backdrop on this screen's own Scaffold. True when
     * the screen is the window root (standalone activity); false when embedded in
     * MainShell, which already paints a single full-window gradient behind everything
     * (avoids a per-screen vs shell-sized double-paint seam at the nav-bar edge).
     */
    paintCanvasBackdrop: Boolean = true,
    /**
     * MainShell D7 edge-to-edge backdrop (S5 carried task): extra reserved
     * space added on top of this screen's OWN Scaffold bottom inset for the
     * SCROLLABLE list content only (see [HistoryList]'s `contentPadding`) —
     * e.g. the floating nav pill's measured footprint when embedded in
     * MainShell. MainShell no longer reserves this space via an outer
     * `Modifier.padding` around the whole screen, so this screen's own pixels
     * can pass BEHIND the pill for its backdrop blur to sample. Zero for the
     * standalone `HistoryActivity` (`paintCanvasBackdrop = true`), which has
     * no floating pill.
     */
    bottomContentPadding: Dp = 0.dp,
) {
    val items by viewModel.items.observeAsState(emptyList())
    val loading by viewModel.loading.observeAsState(false)
    val error by viewModel.errors.observeAsState(null)
    // CopyPaste-yel4: observe the dedicated clearAll error channel.
    val clearAllError by viewModel.clearAllError.observeAsState(null)
    val totalCount by viewModel.totalCount.observeAsState(0)
    val hasMore by viewModel.hasMore.observeAsState(false)
    // §8 glass toast (replaces Material Snackbar): bottom-center glass surface
    // with a leading semantic dot + slide-up. Driven through GlassToastState the
    // same way SnackbarHostState was (scope.launch { toastState.show(...) }).
    // GlassToast now supports action buttons, so the UNDO affordance is handled
    // here too (replaces the previous separate SnackbarHostState / SnackbarHost).
    val toastState = remember { GlassToastState() }
    val scope = rememberCoroutineScope()
    val ctx = LocalContext.current
    val settings = remember { Settings(ctx) }
    // Single shared ClipboardRepository instance for all coroutine lambdas in this
    // composable (file picker, bulk-copy, save/open file, preview overlay actions).
    // Previously constructed 4× inline inside coroutine launches; now created once
    // and captured by all lambdas. Behaviour is identical — same constructor args.
    val repository = remember { ClipboardRepository(ctx) }
    // PARITY-SPEC §1: the active (light-first) ramp — read once at screen scope and
    // reuse for every token below so the chrome (scaffold, top bar, dialogs) themes
    // light/dark in lockstep with SecureWindowChrome.
    val c = MaterialTheme.colorScheme
    // CopyPaste-7m6r: loadErrorTemplate / clearAllErrorTemplate removed — error strings
    // are now routed through ErrorMessages.friendlyOperationError (no raw-msg formatting).
    val sensitiveTapMsg = stringResource(R.string.sensitive_tap_hint)

    // CopyPaste-vp63.37: hoisted search/filter/selection/reorder/preview/confirm
    // state — see HistoryScreenState.kt. Every field is byte-identical (same
    // Saver, same initial value) to what used to be declared inline here.
    val state = rememberHistoryScreenState(settings)

    // ── In-app file picker (HB-11) ───────────────────────────────────────────
    // Opens the system file picker via ACTION_OPEN_DOCUMENT. On a successful pick
    // the URI is routed through the same captureFileClip path the share-target uses,
    // so the file lands in history and is pushed to all active sync transports.
    val fileCapturedMsg = stringResource(R.string.snackbar_file_captured)
    val filePickFailed  = stringResource(R.string.error_file_pick_failed)
    val filePickLauncher = rememberHistoryFilePickerLauncher(
        ctx = ctx,
        settings = settings,
        repository = repository,
        viewModel = viewModel,
        scope = scope,
        onCaptured = { toastState.show(fileCapturedMsg, GlassToastKind.SUCCESS) },
        onFailed = { toastState.show(filePickFailed, GlassToastKind.DANGER) },
    )

    // CopyPaste-vp63.37: reorder/selection BackHandlers + preview/selection
    // LaunchedEffects, moved verbatim to HistoryScreenState.kt.
    HistoryScreenEffects(items, state)

    // CopyPaste-vp63.37: sort/search/device-filter pipeline, moved verbatim to
    // HistoryScreenState.kt (rememberHistoryItemPipelines). sortHistoryItems /
    // filterHistoryItemsBySearch are pure + unit-tested (HistoryScreenStateTest).
    val pipelines = rememberHistoryItemPipelines(items, state, settings, repository)
    val ownDeviceId = pipelines.ownDeviceId
    val pairedPeers = pipelines.pairedPeers
    val sortedItems = pipelines.sortedItems
    val deviceFilteredItems = pipelines.deviceFilteredItems
    val originDeviceIds = pipelines.originDeviceIds

    LaunchedEffect(Unit) { viewModel.loadItems() }

    LaunchedEffect(error) {
        val msg = error ?: return@LaunchedEffect
        // CopyPaste-7m6r: route raw exception message through ErrorMessages so
        // internals (SQLite class names, file-system paths) are never shown.
        toastState.show(ErrorMessages.friendlyOperationError(msg), GlassToastKind.DANGER)
        // android-history 5.3: `errors` is a TRANSIENT signal (cleared right
        // below) — it cannot itself satisfy "a persistent error/degraded state
        // ... not communicated solely via a transient toast" (spec.md). Latch
        // a STICKY presentation-layer flag instead; see `HistoryScreenState`
        // and `HistoryErrorState`'s render branch further down. NO
        // ClipboardViewModel/repository/IPC change — presentation-layer only.
        state.isDegraded = true
        viewModel.clearError()
    }

    // Clears the sticky degraded flag once a subsequent load actually returns
    // data — see `clearsDegradedState` (HistoryScreenState.kt) for the pure
    // decision and its documented limitation (a load that legitimately settles
    // on zero items after an error stays degraded until the user retries).
    LaunchedEffect(loading, sortedItems) {
        if (clearsDegradedState(loading = loading, hasItems = sortedItems.isNotEmpty())) {
            state.isDegraded = false
        }
    }

    // CopyPaste-yel4: clearAll errors are surfaced through a dedicated channel so the
    // message reads "Failed to clear history" instead of the generic load-history text.
    LaunchedEffect(clearAllError) {
        val msg = clearAllError ?: return@LaunchedEffect
        // CopyPaste-7m6r: sanitise raw error — do not expose internals in the toast.
        toastState.show(ErrorMessages.friendlyOperationError(msg), GlassToastKind.DANGER)
        viewModel.clearClearAllError()
    }

    // ── Confirmation dialog ──────────────────────────────────────────────────
    state.pendingConfirm?.let { action ->
        ConfirmationDialog(
            action = action,
            itemCount = when (action) {
                ConfirmAction.CLEAR_UNPINNED -> items.count { !it.pinned }
                ConfirmAction.CLEAR_ALL -> items.size
                ConfirmAction.DELETE_SELECTED -> state.selectedIds.size
                // CopyPaste-2ifa: single-item delete always shows count=1.
                ConfirmAction.DELETE_SINGLE -> 1
            },
            onConfirm = {
                state.pendingConfirm = null
                when (action) {
                    ConfirmAction.CLEAR_UNPINNED -> viewModel.clearUnpinned()
                    ConfirmAction.CLEAR_ALL -> viewModel.clearAll()
                    ConfirmAction.DELETE_SELECTED -> {
                        viewModel.deleteItems(state.selectedIds.toList())
                        state.selectionMode = false
                        state.selectedIds = emptySet()
                    }
                    // CopyPaste-2ifa + CopyPaste-kaf6: confirmed single delete:
                    // show a 5-second GlassToast with an UNDO action button. If the
                    // user taps UNDO within that window the toast dismisses immediately
                    // and the delete is skipped; otherwise the delete is committed after
                    // show() returns (macOS parity — 5-second undo window).
                    ConfirmAction.DELETE_SINGLE -> {
                        val idToDelete = state.pendingDeleteId
                        state.pendingDeleteId = null
                        if (idToDelete != null) {
                            scope.launch {
                                var undone = false
                                toastState.show(
                                    message = ctx.getString(R.string.snackbar_item_deleted),
                                    kind = GlassToastKind.INFO,
                                    durationMs = 5_000L,
                                    action = ctx.getString(R.string.snackbar_undo) to { undone = true },
                                )
                                // show() suspends for durationMs (or until action dismisses it early).
                                // If the action was clicked, undone=true and we skip the delete.
                                if (!undone) {
                                    viewModel.deleteItem(idToDelete)
                                }
                            }
                        }
                    }
                }
            },
            onDismiss = {
                state.pendingConfirm = null
                // CopyPaste-2ifa: if the user cancels the single-delete confirm, clear the
                // pending id so a stale id does not affect future interactions.
                if (action == ConfirmAction.DELETE_SINGLE) state.pendingDeleteId = null
            },
        )
    }

    Scaffold(
        // Calm screen backdrop (STYLEGUIDE §6). When embedded in
        // MainShell (paintCanvasBackdrop=false) the shell already paints it.
        modifier = modifier,
        containerColor = c.background,
        topBar = {
            if (state.selectionMode) {
                val bulkCopiedMsg = stringResource(R.string.snackbar_bulk_copied)
                val bulkCopiedNoTextMsg = stringResource(R.string.snackbar_bulk_copied_no_text)
                SelectionTopBar(
                    selectedCount = state.selectedIds.size,
                    totalCount = sortedItems.size,
                    onClose = {
                        state.selectionMode = false
                        state.selectedIds = emptySet()
                    },
                    onSelectAll = {
                        state.selectedIds = if (state.selectedIds.size == sortedItems.size) {
                            emptySet()
                        } else {
                            sortedItems.map { it.id }.toSet()
                        }
                    },
                    onDeleteSelected = {
                        if (state.selectedIds.isNotEmpty()) {
                            state.pendingConfirm = ConfirmAction.DELETE_SELECTED
                        }
                    },
                    onPinSelected = {
                        state.selectedIds.forEach { id ->
                            val item = sortedItems.find { it.id == id }
                            if (item != null && !item.pinned) viewModel.setPinned(id, true)
                        }
                        state.selectionMode = false
                        state.selectedIds = emptySet()
                    },
                    onUnpinSelected = {
                        state.selectedIds.forEach { id ->
                            val item = sortedItems.find { it.id == id }
                            if (item != null && item.pinned) viewModel.setPinned(id, false)
                        }
                        state.selectionMode = false
                        state.selectedIds = emptySet()
                    },
                    // g3z4: bulk-copy — collect selected text items (sorted by recency,
                    // sensitive items skipped — mirrors desktop Copy All semantics),
                    // join with "\n\n", and set as the system clipboard primary clip.
                    // CopyPaste-vp63.37: body extracted to HistoryItemActions.kt
                    // (bulkCopySelectedText) — pure selection filter is unit-tested
                    // (HistoryItemActionsTest).
                    onCopySelected = {
                        val ids = state.selectedIds
                        scope.launch {
                            val copiedCount = bulkCopySelectedText(ctx, repository, settings, sortedItems, ids)
                            if (copiedCount == 0) {
                                toastState.show(bulkCopiedNoTextMsg, GlassToastKind.INFO)
                            } else {
                                toastState.show(
                                    bulkCopiedMsg.format(copiedCount),
                                    GlassToastKind.SUCCESS,
                                )
                            }
                            state.selectionMode = false
                            state.selectedIds = emptySet()
                        }
                    },
                )
            } else {
                HistoryNormalTopBar(
                    c = c,
                    totalCount = totalCount,
                    showBackButton = showBackButton,
                    onBack = onBack,
                    items = items,
                    sortByDevice = state.sortByDevice,
                    onSortByDeviceChange = { state.sortByDevice = it },
                    settings = settings,
                    searchExpanded = state.searchExpanded,
                    onSearchExpandedChange = { state.searchExpanded = it },
                    searchQuery = state.searchQuery,
                    onSearchQueryChange = { state.searchQuery = it },
                    recentSearches = state.recentSearches,
                    onRecentSearchesChange = { state.recentSearches = it },
                    reorderMode = state.reorderMode,
                    onReorderModeChange = { state.reorderMode = it },
                    overflowExpanded = state.overflowExpanded,
                    onOverflowExpandedChange = { state.overflowExpanded = it },
                    onClearUnpinned = { state.pendingConfirm = ConfirmAction.CLEAR_UNPINNED },
                    onClearAll = { state.pendingConfirm = ConfirmAction.CLEAR_ALL },
                    onFilePick = { filePickLauncher.launch(arrayOf("*/*")) },
                    onLoadItems = { viewModel.loadItems() },
                    originDeviceIds = originDeviceIds,
                    deviceFilter = state.deviceFilter,
                    onDeviceFilterChange = { state.deviceFilter = it },
                    ownDeviceId = ownDeviceId,
                    peers = pairedPeers,
                )
            }
        },
    ) { innerPadding ->
        // The preview overlay must be a sibling of the list inside this Box so
        // the long-press drag gesture remains one continuous pointer stream
        // (not interrupted by a Dialog/Popup window boundary). The overlay uses
        // WindowInsets.statusBars top padding to ensure the card is never occluded
        // by the status bar or app header.
        // MainShell D7 edge-to-edge backdrop: fold [bottomContentPadding] into
        // the bottom inset handed to the list/loading/empty states, on top of
        // this Scaffold's own (top-bar) inset — see [HistoryList]'s own
        // Modifier-vs-contentPadding split for where the edge-to-edge behaviour
        // actually takes effect.
        val ld = LocalLayoutDirection.current
        val listPadding = PaddingValues(
            start = innerPadding.calculateStartPadding(ld),
            top = innerPadding.calculateTopPadding(),
            end = innerPadding.calculateEndPadding(ld),
            bottom = innerPadding.calculateBottomPadding() + bottomContentPadding,
        )
        Box(modifier = Modifier.fillMaxSize()) {
            when {
                loading && sortedItems.isEmpty() -> LoadingBox(listPadding)
                // android-history 5.3 — NEW persistent error/degraded state: only
                // takes over the list surface when there is nothing else to show
                // (a transient blip while valid cached items are still on screen
                // does not blow away that data — see `state.isDegraded`'s kdoc).
                sortedItems.isEmpty() && state.isDegraded ->
                    HistoryErrorState(listPadding, onRetry = { viewModel.loadItems() })
                // §9: history completely empty — CopyPaste-crh3.31: show the
                // private-mode message when recording is paused (parity w/ macOS).
                sortedItems.isEmpty() -> EmptyHistoryState(listPadding, isPrivateMode = settings.privateMode)
                // §9: search returned no results (counting device filter too)
                deviceFilteredItems.isEmpty() -> EmptySearchState(listPadding, state.searchQuery.trim())
                else -> HistoryList(
                    items = deviceFilteredItems,
                    padding = listPadding,
                    hasMore = hasMore,
                    onLoadMore = { viewModel.loadMore() },
                    ownDeviceId = ownDeviceId,
                    peers = pairedPeers,
                    selectionMode = state.selectionMode,
                    selectedIds = state.selectedIds,
                    reorderMode = state.reorderMode,
                    // CopyPaste-2ifa: route single-item delete through a confirmation dialog
                    // instead of deleting immediately. Store the id and set the pending action.
                    onDelete = { id ->
                        state.pendingDeleteId = id
                        state.pendingConfirm = ConfirmAction.DELETE_SINGLE
                    },
                    onSetPinned = { id, pinned -> viewModel.setPinned(id, pinned) },
                    onReorderPinned = { id, direction ->
                        val pinnedItems = sortedItems.filter { it.pinned }
                        val idx = pinnedItems.indexOfFirst { it.id == id }
                        if (idx < 0) return@HistoryList
                        val swapIdx = idx + direction
                        if (swapIdx < 0 || swapIdx >= pinnedItems.size) return@HistoryList
                        val newOrder = pinnedItems.toMutableList().also {
                            val tmp = it[idx]; it[idx] = it[swapIdx]; it[swapIdx] = tmp
                        }
                        viewModel.reorderPinned(newOrder.map { it.id })
                    },
                    onCopied = { id -> viewModel.copyItem(id) },
                    onLongPress = { id ->
                        // Long-press enters selection mode when preview is not active.
                        state.selectionMode = true
                        state.selectedIds = setOf(id)
                    },
                    onCheckboxTap = { id ->
                        if (!state.selectionMode) state.selectionMode = true
                        state.selectedIds = if (state.selectedIds.contains(id)) {
                            val next = state.selectedIds - id
                            if (next.isEmpty()) { state.selectionMode = false }
                            next
                        } else {
                            state.selectedIds + id
                        }
                    },
                    onSensitiveTap = {
                        scope.launch { toastState.show(sensitiveTapMsg, GlassToastKind.INFO) }
                    },
                    // CopyPaste-vp63.37: body extracted to HistoryItemActions.kt
                    // (saveFileToDownloads) — the list-row path sanitizes the
                    // fallback filename (sanitizeFileName = true), matching the
                    // original inline onSaveFile body exactly.
                    onSaveFile = { id ->
                        scope.launch {
                            val saved = saveFileToDownloads(ctx, repository, id, sanitizeFileName = true)
                            if (saved) {
                                toastState.show(ctx.getString(R.string.file_saved_ok), GlassToastKind.SUCCESS)
                            } else {
                                toastState.show(ctx.getString(R.string.file_save_failed), GlassToastKind.DANGER)
                            }
                        }
                    },
                    // CopyPaste-vp63.37: body extracted to HistoryItemActions.kt
                    // (resolveFileForOpen / openResolvedFile).
                    onOpenFile = { id ->
                        scope.launch {
                            val resolution = resolveFileForOpen(ctx, repository, id, logSource = "openFile")
                            if (resolution.opened) {
                                openResolvedFile(ctx, repository, id, resolution) {
                                    toastState.show(ctx.getString(R.string.file_open_no_app), GlassToastKind.DANGER)
                                }
                            } else {
                                toastState.show(resolution.nameOrError, GlassToastKind.DANGER)
                            }
                        }
                    },
                    onPreviewPeek = { id ->
                        state.previewItemId = id
                        state.previewPhase = PreviewPhase.Peeking
                    },
                    onPreviewPin = { id ->
                        state.previewItemId = id
                        state.previewPhase = PreviewPhase.Pinned
                    },
                    onPreviewDismiss = {
                        state.previewItemId = null
                        state.previewPhase = PreviewPhase.Idle
                    },
                    // CopyPaste-5917.76: image/file tapped with paste-as-plain-text ON —
                    // notify via GlassToast and leave clipboard unchanged.
                    onMediaCopyAsText = { msg ->
                        scope.launch { toastState.show(msg, GlassToastKind.INFO) }
                    },
                )
            }

        // ── Preview overlay — in-tree sibling of the list, never a Dialog/Popup ──
        // The overlay applies WindowInsets.statusBars top padding to ensure the card
        // is never occluded by the status bar or app header on any device.
        val previewItem = remember(state.previewItemId, sortedItems) {
            state.previewItemId?.let { id -> sortedItems.find { it.id == id } }
        }
        // previewRepository reuses the single shared `repository` instance defined above.
        PreviewOverlay(
            phase = state.previewPhase,
            item = previewItem,
            repository = repository,
            settings = settings,
            maskSensitive = settings.maskSensitiveContent,
            onDismiss = {
                state.previewItemId = null
                state.previewPhase = PreviewPhase.Idle
            },
            // CopyPaste-vp63.37: body extracted to HistoryItemActions.kt (copyPreviewItem).
            onCopy = {
                val item = previewItem ?: return@PreviewOverlay
                scope.launch {
                    copyPreviewItem(ctx, repository, settings, item)
                    viewModel.copyItem(item.id)
                }
            },
            onSetPinned = { pinned ->
                val id = state.previewItemId ?: return@PreviewOverlay
                viewModel.setPinned(id, pinned)
            },
            onDelete = {
                // CopyPaste-2ifa: route preview overlay delete through the same
                // confirmation dialog as the row delete button.
                val id = state.previewItemId ?: return@PreviewOverlay
                state.pendingDeleteId = id
                state.pendingConfirm = ConfirmAction.DELETE_SINGLE
            },
            // CopyPaste-vp63.37: body extracted to HistoryItemActions.kt
            // (saveFileToDownloads) — the preview path does NOT sanitize the
            // fallback filename (sanitizeFileName = false), preserving the
            // original inline preview onSaveFile body's pre-existing behaviour
            // exactly (see HistoryItemActions.kt's doc comment on that divergence).
            onSaveFile = {
                val id = state.previewItemId ?: return@PreviewOverlay
                scope.launch {
                    val saved = saveFileToDownloads(ctx, repository, id, sanitizeFileName = false)
                    if (saved) {
                        toastState.show(ctx.getString(R.string.file_saved_ok), GlassToastKind.SUCCESS)
                    } else {
                        toastState.show(ctx.getString(R.string.file_save_failed), GlassToastKind.DANGER)
                    }
                }
            },
            // CopyPaste-vp63.37: body extracted to HistoryItemActions.kt
            // (resolveFileForOpen / openResolvedFile).
            onOpenFile = {
                val id = state.previewItemId ?: return@PreviewOverlay
                scope.launch {
                    val resolution = resolveFileForOpen(ctx, repository, id, logSource = "preview openFile")
                    if (resolution.opened) {
                        openResolvedFile(ctx, repository, id, resolution) {
                            toastState.show(ctx.getString(R.string.file_open_no_app), GlassToastKind.DANGER)
                        }
                    } else {
                        toastState.show(resolution.nameOrError, GlassToastKind.DANGER)
                    }
                }
            },
        )

        // §8 glass toast host — overlays the list bottom-center. Inside this Box
        // so it floats above the history content (replaces the Scaffold's
        // Material SnackbarHost).
        GlassToastHost(state = toastState)
        } // end Box
    }
}
