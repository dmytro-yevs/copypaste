@file:OptIn(ExperimentalFoundationApi::class)

package com.copypaste.android

import android.content.pm.PackageManager
import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.slideInHorizontally
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.livedata.observeAsState
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import kotlinx.coroutines.launch
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import android.content.ClipData
import android.content.ClipboardManager
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.os.Environment
import android.provider.MediaStore
import androidx.core.content.FileProvider
import java.io.File
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.copypaste.android.ui.GlassToastHost
import com.copypaste.android.ui.GlassToastKind
import com.copypaste.android.ui.GlassToastState
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.auroraCanvas
import com.copypaste.android.ui.theme.isDarkTheme
import com.copypaste.android.ui.theme.tintBlobCanvas
import com.copypaste.android.ui.theme.rememberTranslucency
import com.copypaste.android.ui.theme.EaseOutExpo
import com.copypaste.android.ui.theme.rememberReducedMotion
// PARITY-SPEC §1: read the ACTIVE (light-first) ramp via LocalIdeColors.current.*
// instead of the hardcoded dark Ide* constants, so the whole History screen
// themes light/dark in lockstep with CopyPasteTheme. The IdeColors holder is
// passed into non-composable helpers (e.g. the chip color table) by value.
import com.copypaste.android.ui.theme.IdeColors
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.Motion
// Liquid glass / palette tokens for aurora backdrop and cinematic motion.
import com.copypaste.android.ui.theme.LocalPalette
import com.copypaste.android.ui.theme.motionDuration
import com.copypaste.android.ui.theme.paletteAurora
// A-C1: skin axis tokens for screen-level treatment (background, row, nav).
import com.copypaste.android.ui.theme.LocalSkin
import com.copypaste.android.ui.theme.SkinBackground
import com.copypaste.android.ui.theme.SkinRowTreatment
import com.copypaste.android.ui.theme.SkinTokens
import com.copypaste.android.ui.theme.skinTokens
import kotlinx.coroutines.delay
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts

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
 */
class HistoryActivity : ComponentActivity() {

    private val viewModel: ClipboardViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // CopyPaste-1g00: screenshot protection is now pref-driven (Settings.allowScreenshots).
        // CopyPasteTheme applies FLAG_SECURE centrally when allowScreenshots=false (the default).
        // The old hardcoded setFlags(FLAG_SECURE) is removed so the user's pref is honoured.
        applyScreenshotPolicy(Settings(this))
        enableEdgeToEdge()

        // CopyPaste-0qpn: wire the mutation sync hook so pin/unpin/reorder/delete/clear
        // operations propagate to peers over relay + Supabase. Delegates to
        // ClipboardService.requestMutationQueueDrain which fires a drain on the service's
        // IO scope (non-blocking, fire-and-forget). Hook is a no-op when FGS is not running.
        viewModel.onMutationSync = {
            ClipboardService.requestMutationQueueDrain()
        }

        setContent {
            CopyPasteTheme {
                HistoryScreen(
                    viewModel = viewModel,
                    onBack = { finish() }
                )
            }
        }
    }

    companion object {
        /** Fallback used only when Settings cannot be read (e.g. test context). */
        const val HISTORY_LIMIT = 50
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Screen
// ─────────────────────────────────────────────────────────────────────────────

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HistoryScreen(
    viewModel: ClipboardViewModel = viewModel(),
    modifier: Modifier = Modifier,
    showBackButton: Boolean = true,
    onBack: () -> Unit = {},
    /**
     * §1: paint the aurora canvas backdrop on this screen's own Scaffold. True when
     * the screen is the window root (standalone activity); false when embedded in
     * MainShell, which already paints a single full-window aurora behind everything
     * (avoids a per-screen vs shell-sized double-paint seam at the nav-bar edge).
     */
    paintCanvasBackdrop: Boolean = true,
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
    // light/dark in lockstep with CopyPasteTheme.
    val c = LocalIdeColors.current
    // A-C1: skin tokens for screen-level treatment (background, row, nav).
    val tok = skinTokens(LocalSkin.current)
    // §8 a11y: skip animated transitions when the user has requested reduced motion
    // (Accessibility → Remove animations, or Developer Options → Animator duration scale = 0).
    val reducedMotion = rememberReducedMotion()
    // §2/P0: glass pref + theme for the frosted header (LiquidGlassSurface).
    val translucent = rememberTranslucency()
    val dark = isDarkTheme()
    // CopyPaste-7m6r: loadErrorTemplate / clearAllErrorTemplate removed — error strings
    // are now routed through ErrorMessages.friendlyOperationError (no raw-msg formatting).
    val sensitiveTapMsg = stringResource(R.string.sensitive_tap_hint)

    // ── In-app file picker (HB-11) ───────────────────────────────────────────
    // Opens the system file picker via ACTION_OPEN_DOCUMENT. On a successful pick
    // the URI is routed through the same captureFileClip path the share-target uses,
    // so the file lands in history and is pushed to all active sync transports.
    val fileCapturedMsg = stringResource(R.string.snackbar_file_captured)
    val filePickFailed  = stringResource(R.string.error_file_pick_failed)
    val filePickLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocument(),
    ) { uri: android.net.Uri? ->
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch(kotlinx.coroutines.Dispatchers.IO) {
            try {
                val syncManager = try {
                    SyncManager(
                        RelayClient(settings.relayUrl),
                        settings.deviceId,
                        token = "",
                        settings = settings,
                    )
                } catch (_: Exception) { null }
                val mime = ctx.contentResolver.getType(uri) ?: "application/octet-stream"
                ClipboardService.captureFileClip(
                    context = ctx,
                    uri = uri,
                    mimeType = mime,
                    settings = settings,
                    repository = repository,
                    syncManager = syncManager,
                )
                withContext(kotlinx.coroutines.Dispatchers.Main) {
                    toastState.show(fileCapturedMsg, GlassToastKind.SUCCESS)
                }
                viewModel.loadItems()
            } catch (t: Throwable) {
                withContext(kotlinx.coroutines.Dispatchers.Main) {
                    toastState.show(filePickFailed, GlassToastKind.DANGER)
                }
            }
        }
    }

    // ── Search / filter state ────────────────────────────────────────────────
    var searchQuery by rememberSaveable { mutableStateOf("") }
    // HW-A8: icon-toggle search bar — expanded state + last-5 recent queries.
    var searchExpanded by rememberSaveable { mutableStateOf(false) }
    // Recent searches are PERSISTED in Settings (SharedPreferences), not just
    // across rotation — so they survive process death. Seed once from settings.
    var recentSearches by remember { mutableStateOf(settings.recentSearches) }

    // ── Device filter (parity with macOS HistoryView deviceFilter) ───────────
    // "all" = no filter; any other value = UUID of the origin device to show.
    // Reset to "all" when the set of known devices shrinks (e.g. after clearing
    // all items from a peer device) so we never show an empty filter.
    var deviceFilter by rememberSaveable { mutableStateOf("all") }

    // ── Selection state (survives rotation) ─────────────────────────────────
    var selectionMode by rememberSaveable { mutableStateOf(false) }
    var selectedIds by rememberSaveable(
        stateSaver = listSaver(
            save    = { it.toList() },
            restore = { it.toSet() },
        )
    ) { mutableStateOf(setOf<String>()) }

    // rememberSaveable so dialog/menu state survives rotation (fix P2).
    // ConfirmAction is an enum — saved as its ordinal Int.
    var pendingConfirm by rememberSaveable(
        stateSaver = androidx.compose.runtime.saveable.Saver(
            save    = { it?.ordinal },
            restore = { ord -> ConfirmAction.entries.getOrNull(ord) },
        )
    ) { mutableStateOf<ConfirmAction?>(null) }
    // CopyPaste-2ifa: id of the single item whose delete is waiting for confirmation.
    // Cleared when the dialog is dismissed or after deletion starts.
    var pendingDeleteId by rememberSaveable { mutableStateOf<String?>(null) }
    var overflowExpanded by rememberSaveable { mutableStateOf(false) }

    // ── Reorder mode (pinned items only) ────────────────────────────────────
    var reorderMode by rememberSaveable { mutableStateOf(false) }

    // CopyPaste-un29: Sort / group by device — persisted in Settings so the
    // user's choice survives process death (like density/theme). Seeded from
    // prefs once; toggled via the overflow menu and written back immediately.
    var sortByDevice by rememberSaveable { mutableStateOf(settings.sortByDevice) }

    BackHandler(enabled = reorderMode) { reorderMode = false }

    // ── Long-press peek preview state ────────────────────────────────────────
    // previewItemId + previewPhase are rememberSaveable so a pinned preview
    // survives rotation.  The overlay re-triggers its lazy load on restore via
    // key = item.id + phase in produceState inside PreviewOverlay.
    var previewItemId by rememberSaveable { mutableStateOf<String?>(null) }
    var previewPhase by rememberSaveable(
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

    // Auto-dismiss when the previewed item is no longer in the list.
    LaunchedEffect(items, previewItemId) {
        val id = previewItemId ?: return@LaunchedEffect
        if (items.none { it.id == id }) {
            previewItemId = null
            previewPhase = PreviewPhase.Idle
        }
    }

    // Entering selection mode collapses any open preview.
    LaunchedEffect(selectionMode) {
        if (selectionMode && previewPhase != PreviewPhase.Idle) {
            previewItemId = null
            previewPhase = PreviewPhase.Idle
        }
    }

    // ── Device identity — needed for sort-by-device and device-filter ───────────
    // Defined before sortedItems because sortByDevice sort references them.
    val ownDeviceId = remember { settings.deviceId }
    val pairedPeers = remember { settings.pairedPeers }

    // Sort: pinned first (by user-defined pinnedSortIndex), then unpinned by recency.
    // Pinned items are sorted by pinnedSortIndex (NOT wallTimeMs) so copying a pinned
    // clip does not move it — fixes HW-A15.
    //
    // CopyPaste-un29: when sortByDevice is true, group unpinned items by origin device
    // (own device first, then peers alphabetically by display name, null-origin last),
    // then by recency within each device group — mirrors macOS HistoryView device sort.
    // Pinned items always remain at the top in user-defined order regardless of the sort.
    val sortedItems = remember(items, sortByDevice, ownDeviceId, pairedPeers) {
        // Defensive de-dup by id BEFORE the list reaches the LazyColumn. The list
        // backing the LazyColumn uses `key = { it.id }`, so a duplicate id throws
        // IllegalArgumentException ("Key … was already used") and crash-loops the
        // screen. A persistent duplicate can arise in the repository id index (e.g.
        // a synced item re-appended under the same overrideId after the
        // synced-source-id seen-set was cleared by clearUnpinned). Collapsing
        // duplicates here guarantees the LazyColumn can never crash regardless of
        // how the backing store drifts; the repository fix below removes the source.
        val deduped = items.distinctBy { it.id }
        if (sortByDevice) {
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
    // ── AB-11: full-content search ───────────────────────────────────────────
    // The snippet-only filter missed any match past the 140-char preview. We now
    // ALSO match the full decrypted text. To stay responsive we (a) show instant
    // snippet matches synchronously, and (b) compute full-content matches in the
    // background (debounced) and union them in once ready. Result: typing feels
    // immediate and deep matches surface shortly after.
    // searchRepository reuses the single shared `repository` instance defined above.
    var fullMatchIds by remember { mutableStateOf<Set<String>>(emptySet()) }
    var fullMatchQuery by remember { mutableStateOf("") }

    // F: key only on searchQuery (not sortedItems) so the effect does not re-fire
    // on every list re-emit when the query is empty — the common case after A+B
    // eliminate no-op emits. When query is non-empty we also hash the id list so
    // a new item appearing while searching still triggers a fresh full-content scan.
    val idListHash = remember(sortedItems) { sortedItems.map { it.id }.hashCode() }
    LaunchedEffect(searchQuery, if (searchQuery.isBlank()) 0 else idListHash) {
        val q = searchQuery.trim()
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
    val filteredItems = remember(sortedItems, searchQuery, fullMatchIds, fullMatchQuery) {
        val q = searchQuery.trim()
        if (q.isEmpty()) {
            sortedItems
        } else {
            // Only trust fullMatchIds when it was computed for the CURRENT query;
            // otherwise fall back to the snippet match alone until it catches up.
            val useFull = fullMatchQuery == q
            sortedItems.filter { item ->
                item.snippet.contains(q, ignoreCase = true) ||
                    (useFull && item.id in fullMatchIds)
            }
        }
    }

    // ── Device filter (parity with macOS) ────────────────────────────────────
    // Collect distinct origin device ids from the FULL sorted list (not search-
    // filtered) so the filter chips are stable while typing. Show the chips only
    // when more than one device is present — mirrors macOS HistoryView.
    val originDeviceIds = remember(sortedItems) { distinctOriginDeviceIds(sortedItems) }

    // Auto-reset device filter when the selected device disappears from the list
    // (e.g. all items from that device were deleted).
    LaunchedEffect(originDeviceIds, deviceFilter) {
        if (deviceFilter != "all" && deviceFilter !in originDeviceIds) {
            deviceFilter = "all"
        }
    }

    // Apply device filter on top of search filter.
    val deviceFilteredItems = remember(filteredItems, deviceFilter) {
        filterByDevice(filteredItems, deviceFilter)
    }

    BackHandler(enabled = selectionMode) {
        selectionMode = false
        selectedIds = emptySet()
    }

    // Entering selection mode exits reorder mode and collapses any open preview
    LaunchedEffect(selectionMode) {
        if (selectionMode) {
            reorderMode = false
            // Collapse preview when selection mode activates
            if (previewPhase != PreviewPhase.Idle) {
                previewItemId = null
                previewPhase = PreviewPhase.Idle
            }
        }
    }

    // Drop selected ids that no longer exist when the underlying list changes
    // (background sync eviction, prune, TTL, remote delete) so the selected
    // count stays accurate. Intersect against the FULL `items` list — not the
    // search-filtered view — so selected-but-hidden items are not wrongly lost.
    LaunchedEffect(items) {
        if (selectionMode) {
            val currentIds = items.mapTo(HashSet()) { it.id }
            val pruned = selectedIds.intersect(currentIds)
            if (pruned.size != selectedIds.size) {
                selectedIds = pruned
                if (pruned.isEmpty()) selectionMode = false
            }
        }
    }

    LaunchedEffect(Unit) { viewModel.loadItems() }

    LaunchedEffect(error) {
        val msg = error ?: return@LaunchedEffect
        // CopyPaste-7m6r: route raw exception message through ErrorMessages so
        // internals (SQLite class names, file-system paths) are never shown.
        toastState.show(ErrorMessages.friendlyOperationError(msg), GlassToastKind.DANGER)
        viewModel.clearError()
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
    pendingConfirm?.let { action ->
        ConfirmationDialog(
            action = action,
            itemCount = when (action) {
                ConfirmAction.CLEAR_UNPINNED -> items.count { !it.pinned }
                ConfirmAction.CLEAR_ALL -> items.size
                ConfirmAction.DELETE_SELECTED -> selectedIds.size
                // CopyPaste-2ifa: single-item delete always shows count=1.
                ConfirmAction.DELETE_SINGLE -> 1
            },
            onConfirm = {
                pendingConfirm = null
                when (action) {
                    ConfirmAction.CLEAR_UNPINNED -> viewModel.clearUnpinned()
                    ConfirmAction.CLEAR_ALL -> viewModel.clearAll()
                    ConfirmAction.DELETE_SELECTED -> {
                        viewModel.deleteItems(selectedIds.toList())
                        selectionMode = false
                        selectedIds = emptySet()
                    }
                    // CopyPaste-2ifa + CopyPaste-kaf6: confirmed single delete:
                    // show a 5-second GlassToast with an UNDO action button. If the
                    // user taps UNDO within that window the toast dismisses immediately
                    // and the delete is skipped; otherwise the delete is committed after
                    // show() returns (macOS parity — 5-second undo window).
                    ConfirmAction.DELETE_SINGLE -> {
                        val idToDelete = pendingDeleteId
                        pendingDeleteId = null
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
                pendingConfirm = null
                // CopyPaste-2ifa: if the user cancels the single-delete confirm, clear the
                // pending id so a stale id does not affect future interactions.
                if (action == ConfirmAction.DELETE_SINGLE) pendingDeleteId = null
            },
        )
    }

    Scaffold(
        // A-C1 + §1 aurora canvas: gated by tok.background to support all three skins.
        //   AURORA    — current full animated aurora (Classic). Painted here when standalone;
        //               the MainShell paints it when embedded (paintCanvasBackdrop=false).
        //   FLAT      — plain solid bg, no aurora (Quiet). The FLAT check precedes translucent
        //               so even with translucency ON the Quiet skin stays solid.
        //   TINT_BLOB — single static accent-tinted blob (Vapor). Painted via a simplified
        //               auroraCanvas call using only the palette's primary glow blob.
        // Classic with paintCanvasBackdrop=false returns the base modifier unchanged —
        // the MainShell backdrop is already in place. FLAT always returns base modifier.
        modifier = when {
            !paintCanvasBackdrop                          -> modifier
            !translucent                                  -> modifier
            tok.background == SkinBackground.AURORA       ->
                modifier.auroraCanvas(dark, paletteAurora(LocalPalette.current))
            tok.background == SkinBackground.TINT_BLOB    ->
                // CopyPaste-uya3: use shared tintBlobCanvas from Components.kt
                // (canonical AboutActivity calibration: base gradient + glowA + glowB + centre).
                modifier.tintBlobCanvas(dark, paletteAurora(LocalPalette.current), tok.glow)
            else                                          -> modifier // FLAT: solid, no canvas
        },
        containerColor = if (translucent) Color.Transparent else c.bg,
        topBar = {
            if (selectionMode) {
                val bulkCopiedMsg = stringResource(R.string.snackbar_bulk_copied)
                val bulkCopiedNoTextMsg = stringResource(R.string.snackbar_bulk_copied_no_text)
                SelectionTopBar(
                    selectedCount = selectedIds.size,
                    totalCount = sortedItems.size,
                    onClose = {
                        selectionMode = false
                        selectedIds = emptySet()
                    },
                    onSelectAll = {
                        selectedIds = if (selectedIds.size == sortedItems.size) {
                            emptySet()
                        } else {
                            sortedItems.map { it.id }.toSet()
                        }
                    },
                    onDeleteSelected = {
                        if (selectedIds.isNotEmpty()) {
                            pendingConfirm = ConfirmAction.DELETE_SELECTED
                        }
                    },
                    onPinSelected = {
                        selectedIds.forEach { id ->
                            val item = sortedItems.find { it.id == id }
                            if (item != null && !item.pinned) viewModel.setPinned(id, true)
                        }
                        selectionMode = false
                        selectedIds = emptySet()
                    },
                    onUnpinSelected = {
                        selectedIds.forEach { id ->
                            val item = sortedItems.find { it.id == id }
                            if (item != null && item.pinned) viewModel.setPinned(id, false)
                        }
                        selectionMode = false
                        selectedIds = emptySet()
                    },
                    // g3z4: bulk-copy — collect selected text items (sorted by recency,
                    // sensitive items skipped — mirrors desktop Copy All semantics),
                    // join with "\n\n", and set as the system clipboard primary clip.
                    onCopySelected = {
                        val ids = selectedIds
                        scope.launch {
                            val key = settings.encryptionKey
                            // Preserve display order: walk sortedItems (pinned-first then
                            // by recency) and retain only selected text items that are
                            // not sensitive. Sensitive items are intentionally excluded
                            // to avoid silently placing credentials in the clipboard.
                            val textItems = sortedItems.filter { item ->
                                item.id in ids && item.isText && !item.isSensitive
                            }
                            if (textItems.isEmpty()) {
                                toastState.show(bulkCopiedNoTextMsg, GlassToastKind.INFO)
                            } else {
                                val parts = withContext(Dispatchers.IO) {
                                    textItems.map { item ->
                                        repository.loadFullPlaintext(item.id, key)
                                            ?: item.snippet
                                    }
                                }
                                val joined = parts.joinToString("\n\n")
                                ClipboardRepository.expectClip(joined)
                                val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE)
                                    as ClipboardManager
                                cm.setPrimaryClip(ClipData.newPlainText("CopyPaste", joined))
                                toastState.show(
                                    bulkCopiedMsg.format(textItems.size),
                                    GlassToastKind.SUCCESS,
                                )
                            }
                            selectionMode = false
                            selectedIds = emptySet()
                        }
                    },
                )
            } else {
                HistoryNormalTopBar(
                    c = c,
                    translucent = translucent,
                    dark = dark,
                    reducedMotion = reducedMotion,
                    totalCount = totalCount,
                    showBackButton = showBackButton,
                    onBack = onBack,
                    items = items,
                    sortByDevice = sortByDevice,
                    onSortByDeviceChange = { sortByDevice = it },
                    settings = settings,
                    searchExpanded = searchExpanded,
                    onSearchExpandedChange = { searchExpanded = it },
                    searchQuery = searchQuery,
                    onSearchQueryChange = { searchQuery = it },
                    recentSearches = recentSearches,
                    onRecentSearchesChange = { recentSearches = it },
                    reorderMode = reorderMode,
                    onReorderModeChange = { reorderMode = it },
                    overflowExpanded = overflowExpanded,
                    onOverflowExpandedChange = { overflowExpanded = it },
                    onClearUnpinned = { pendingConfirm = ConfirmAction.CLEAR_UNPINNED },
                    onClearAll = { pendingConfirm = ConfirmAction.CLEAR_ALL },
                    onFilePick = { filePickLauncher.launch(arrayOf("*/*")) },
                    onLoadItems = { viewModel.loadItems() },
                    originDeviceIds = originDeviceIds,
                    deviceFilter = deviceFilter,
                    onDeviceFilterChange = { deviceFilter = it },
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
        Box(modifier = Modifier.fillMaxSize()) {
            when {
                loading && sortedItems.isEmpty() -> LoadingBox(innerPadding)
                // §9: history completely empty — CopyPaste-crh3.31: show the
                // private-mode message when recording is paused (parity w/ macOS).
                sortedItems.isEmpty() -> EmptyHistoryState(innerPadding, isPrivateMode = settings.privateMode)
                // §9: search returned no results (counting device filter too)
                deviceFilteredItems.isEmpty() -> EmptySearchState(innerPadding, searchQuery.trim())
                else -> HistoryList(
                    items = deviceFilteredItems,
                    padding = innerPadding,
                    tok = tok,
                    hasMore = hasMore,
                    onLoadMore = { viewModel.loadMore() },
                    ownDeviceId = ownDeviceId,
                    peers = pairedPeers,
                    selectionMode = selectionMode,
                    selectedIds = selectedIds,
                    reorderMode = reorderMode,
                    // CopyPaste-2ifa: route single-item delete through a confirmation dialog
                    // instead of deleting immediately. Store the id and set the pending action.
                    onDelete = { id ->
                        pendingDeleteId = id
                        pendingConfirm = ConfirmAction.DELETE_SINGLE
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
                        selectionMode = true
                        selectedIds = setOf(id)
                    },
                    onCheckboxTap = { id ->
                        if (!selectionMode) selectionMode = true
                        selectedIds = if (selectedIds.contains(id)) {
                            val next = selectedIds - id
                            if (next.isEmpty()) { selectionMode = false }
                            next
                        } else {
                            selectedIds + id
                        }
                    },
                    onSensitiveTap = {
                        scope.launch { toastState.show(sensitiveTapMsg, GlassToastKind.INFO) }
                    },
                    onSaveFile = { id ->
                        scope.launch {
                            val saved = withContext(Dispatchers.IO) {
                                try {
                                    // MediaStore.Downloads requires API 29+; devices below that are unsupported.
                                    if (android.os.Build.VERSION.SDK_INT < android.os.Build.VERSION_CODES.Q) return@withContext false
                                    val fileBytes = repository.getFileBytes(id) ?: return@withContext false
                                    val (fileName, mime) = repository.getFileMeta(id)
                                    val rawName = fileName?.takeIf { it.isNotBlank() } ?: "file_$id.bin"
                                    // ouly: sanitize peer-supplied filename before use as MediaStore DISPLAY_NAME —
                                    // strips path-traversal sequences and shell-special chars for consistency with onOpenFile (fr44).
                                    val safeName = FileSecurityHelper.sanitizeFilename(rawName)
                                    val mimeType = mime ?: "application/octet-stream"
                                    // API 29+: insert into MediaStore.Downloads (no WRITE_EXTERNAL_STORAGE needed)
                                    val values = ContentValues().apply {
                                        put(MediaStore.Downloads.DISPLAY_NAME, safeName)
                                        put(MediaStore.Downloads.MIME_TYPE, mimeType)
                                        put(MediaStore.Downloads.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS)
                                        put(MediaStore.Downloads.IS_PENDING, 1)
                                    }
                                    val resolver = ctx.contentResolver
                                    val uri = resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
                                        ?: return@withContext false
                                    resolver.openOutputStream(uri)?.use { it.write(fileBytes) }
                                    values.clear()
                                    values.put(MediaStore.Downloads.IS_PENDING, 0)
                                    resolver.update(uri, values, null, null)
                                    true
                                } catch (e: Exception) {
                                    android.util.Log.w("HistoryActivity", "saveFile failed for $id: ${e.message}")
                                    false
                                }
                            }
                            if (saved) {
                                toastState.show(ctx.getString(R.string.file_saved_ok), GlassToastKind.SUCCESS)
                            } else {
                                toastState.show(ctx.getString(R.string.file_save_failed), GlassToastKind.DANGER)
                            }
                        }
                    },
                    onOpenFile = { id ->
                        // Write file bytes to a cache temp file and open with the OS default app.
                        // Uses the same file_copy FileProvider path as the copy-back flow.
                        // fr44: filename is sanitized and dangerous extensions are blocked.
                        scope.launch {
                            // CopyPaste-ev7z: return safeName from the IO block so the extension
                            // check uses the SANITIZED name, not the raw peer-supplied filename.
                            // Triple: (opened, safeName|errorMsg, uriString|"")
                            val (opened, safeNameOrError, uriStr) = withContext(Dispatchers.IO) {
                                try {
                                    val fileBytes = repository.getFileBytes(id)
                                        ?: return@withContext Triple(false, ctx.getString(R.string.file_save_failed), "")
                                    val (fileName, _) = repository.getFileMeta(id)
                                    // fr44: sanitize the peer-supplied filename before writing to
                                    // disk — strips path-traversal sequences and shell-special chars.
                                    val rawName = fileName?.takeIf { it.isNotBlank() } ?: "file_$id.bin"
                                    val safeName = FileSecurityHelper.sanitizeFilename(rawName)
                                    val dir = File(ctx.cacheDir, "file_copy").also { it.mkdirs() }
                                    val file = File(dir, safeName)
                                    file.writeBytes(fileBytes)
                                    val uri = FileProvider.getUriForFile(
                                        ctx,
                                        "${ctx.packageName}.fileprovider",
                                        file,
                                    )
                                    Triple(true, safeName, uri.toString())
                                } catch (e: Exception) {
                                    android.util.Log.w("HistoryActivity", "openFile failed for $id: ${e.message}")
                                    Triple(false, ctx.getString(R.string.file_save_failed), "")
                                }
                            }
                            if (opened) {
                                // uriStr holds the URI string on success
                                val uri = android.net.Uri.parse(uriStr)
                                val (_, mime) = withContext(Dispatchers.IO) { repository.getFileMeta(id) }
                                // CopyPaste-ev7z: extract extension from safeNameOrError (the sanitized
                                // name) — NOT from rawFileName. Using the raw name allowed a peer to
                                // bypass the denylist via path-traversal or null-byte tricks.
                                val ext = safeNameOrError.substringAfterLast('.', "").lowercase()
                                if (FileSecurityHelper.isDangerousExtension(ext)) {
                                    val shareIntent = Intent(Intent.ACTION_SEND).apply {
                                        type = mime ?: "application/octet-stream"
                                        putExtra(Intent.EXTRA_STREAM, uri)
                                        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                                    }
                                    val chooser = Intent.createChooser(
                                        shareIntent,
                                        ctx.getString(R.string.file_open_dangerous_ext),
                                    ).apply { addFlags(Intent.FLAG_ACTIVITY_NEW_TASK) }
                                    ctx.startActivity(chooser)
                                } else {
                                    val intent = Intent(Intent.ACTION_VIEW).apply {
                                        setDataAndType(uri, mime ?: "*/*")
                                        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                                        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                                    }
                                    // Check if any app can handle this intent before startActivity.
                                    if (ctx.packageManager.resolveActivity(intent, PackageManager.MATCH_DEFAULT_ONLY) != null) {
                                        ctx.startActivity(intent)
                                    } else {
                                        toastState.show(ctx.getString(R.string.file_open_no_app), GlassToastKind.DANGER)
                                    }
                                }
                            } else {
                                toastState.show(safeNameOrError, GlassToastKind.DANGER)
                            }
                        }
                    },
                    onPreviewPeek = { id ->
                        previewItemId = id
                        previewPhase = PreviewPhase.Peeking
                    },
                    onPreviewPin = { id ->
                        previewItemId = id
                        previewPhase = PreviewPhase.Pinned
                    },
                    onPreviewDismiss = {
                        previewItemId = null
                        previewPhase = PreviewPhase.Idle
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
        val previewItem = remember(previewItemId, sortedItems) {
            previewItemId?.let { id -> sortedItems.find { it.id == id } }
        }
        // previewRepository reuses the single shared `repository` instance defined above.
        PreviewOverlay(
            phase = previewPhase,
            item = previewItem,
            repository = repository,
            settings = settings,
            maskSensitive = settings.maskSensitiveContent,
            onDismiss = {
                previewItemId = null
                previewPhase = PreviewPhase.Idle
            },
            onCopy = {
                val item = previewItem ?: return@PreviewOverlay
                scope.launch {
                    val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                    when {
                        item.isImage -> {
                            val imageBytes = withContext(Dispatchers.IO) { repository.getImageBytes(item.id) }
                            if (imageBytes != null) {
                                val uri = withContext(Dispatchers.IO) {
                                    try {
                                        val dir = File(ctx.cacheDir, "image_copy").also { it.mkdirs() }
                                        val file = File(dir, "${item.id}.png")
                                        file.writeBytes(imageBytes)
                                        FileProvider.getUriForFile(ctx, "${ctx.packageName}.fileprovider", file)
                                    } catch (_: Exception) { null }
                                }
                                if (uri != null) {
                                    val clip = ClipData.newUri(ctx.contentResolver, "CopyPaste image", uri)
                                    // CopyPaste-5917.73: narrowed grant — image/png targets only.
                                    grantUriToAll(ctx, uri, "image/png")
                                    cm.setPrimaryClip(clip)
                                }
                            }
                        }
                        item.isFile -> {
                            val fileBytes = withContext(Dispatchers.IO) { repository.getFileBytes(item.id) }
                            if (fileBytes != null) {
                                val uri = withContext(Dispatchers.IO) {
                                    try {
                                        val (fileName, _) = repository.getFileMeta(item.id)
                                        val safeName = fileName?.takeIf { it.isNotBlank() } ?: "${item.id}.bin"
                                        val dir = File(ctx.cacheDir, "file_copy").also { it.mkdirs() }
                                        val file = File(dir, safeName)
                                        file.writeBytes(fileBytes)
                                        FileProvider.getUriForFile(ctx, "${ctx.packageName}.fileprovider", file)
                                    } catch (_: Exception) { null }
                                }
                                if (uri != null) {
                                    val clip = ClipData.newUri(ctx.contentResolver, "CopyPaste file", uri)
                                    // CopyPaste-5917.73: narrowed grant — octet-stream targets only.
                                    grantUriToAll(ctx, uri, "application/octet-stream")
                                    cm.setPrimaryClip(clip)
                                }
                            }
                        }
                        else -> {
                            val fullText = withContext(Dispatchers.IO) {
                                repository.loadFullPlaintext(item.id, settings.encryptionKey)
                            } ?: item.snippet
                            ClipboardRepository.expectClip(fullText)
                            cm.setPrimaryClip(ClipData.newPlainText("CopyPaste", fullText))
                        }
                    }
                    viewModel.copyItem(item.id)
                }
            },
            onSetPinned = { pinned ->
                val id = previewItemId ?: return@PreviewOverlay
                viewModel.setPinned(id, pinned)
            },
            onDelete = {
                // CopyPaste-2ifa: route preview overlay delete through the same
                // confirmation dialog as the row delete button.
                val id = previewItemId ?: return@PreviewOverlay
                pendingDeleteId = id
                pendingConfirm = ConfirmAction.DELETE_SINGLE
            },
            onSaveFile = {
                val id = previewItemId ?: return@PreviewOverlay
                scope.launch {
                    val saved = withContext(Dispatchers.IO) {
                        try {
                            // MediaStore.Downloads requires API 29+; devices below that are unsupported.
                            if (android.os.Build.VERSION.SDK_INT < android.os.Build.VERSION_CODES.Q) return@withContext false
                            val fileBytes = repository.getFileBytes(id) ?: return@withContext false
                            val (fileName, mime) = repository.getFileMeta(id)
                            val safeName = fileName?.takeIf { it.isNotBlank() } ?: "file_$id.bin"
                            val mimeType = mime ?: "application/octet-stream"
                            val values = ContentValues().apply {
                                put(MediaStore.Downloads.DISPLAY_NAME, safeName)
                                put(MediaStore.Downloads.MIME_TYPE, mimeType)
                                put(MediaStore.Downloads.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS)
                                put(MediaStore.Downloads.IS_PENDING, 1)
                            }
                            val resolver = ctx.contentResolver
                            val uri = resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
                                ?: return@withContext false
                            resolver.openOutputStream(uri)?.use { it.write(fileBytes) }
                            values.clear()
                            values.put(MediaStore.Downloads.IS_PENDING, 0)
                            resolver.update(uri, values, null, null)
                            true
                        } catch (e: Exception) {
                            android.util.Log.w("HistoryActivity", "preview saveFile failed for $id: ${e.message}")
                            false
                        }
                    }
                    if (saved) {
                        toastState.show(ctx.getString(R.string.file_saved_ok), GlassToastKind.SUCCESS)
                    } else {
                        toastState.show(ctx.getString(R.string.file_save_failed), GlassToastKind.DANGER)
                    }
                }
            },
            onOpenFile = {
                val id = previewItemId ?: return@PreviewOverlay
                // Open the previewed file with the OS default application.
                // Same implementation as the list-row open action.
                // fr44: filename sanitized; dangerous extensions routed to share chooser.
                scope.launch {
                    // CopyPaste-ev7z: return safeName from the IO block so the extension
                    // check uses the SANITIZED name (same fix as the list-row onOpenFile above).
                    val (opened, safeNameOrError, uriStr) = withContext(Dispatchers.IO) {
                        try {
                            val fileBytes = repository.getFileBytes(id)
                                ?: return@withContext Triple(false, ctx.getString(R.string.file_save_failed), "")
                            val (fileName, _) = repository.getFileMeta(id)
                            // fr44: sanitize peer-supplied filename before writing to disk.
                            val rawName = fileName?.takeIf { it.isNotBlank() } ?: "file_$id.bin"
                            val safeName = FileSecurityHelper.sanitizeFilename(rawName)
                            val dir = File(ctx.cacheDir, "file_copy").also { it.mkdirs() }
                            val file = File(dir, safeName)
                            file.writeBytes(fileBytes)
                            val uri = FileProvider.getUriForFile(
                                ctx,
                                "${ctx.packageName}.fileprovider",
                                file,
                            )
                            Triple(true, safeName, uri.toString())
                        } catch (e: Exception) {
                            android.util.Log.w("HistoryActivity", "preview openFile failed for $id: ${e.message}")
                            Triple(false, ctx.getString(R.string.file_save_failed), "")
                        }
                    }
                    if (opened) {
                        val uri = android.net.Uri.parse(uriStr)
                        val (_, mime) = withContext(Dispatchers.IO) { repository.getFileMeta(id) }
                        // CopyPaste-ev7z: use sanitized name (safeNameOrError) for extension check.
                        val ext = safeNameOrError.substringAfterLast('.', "").lowercase()
                        if (FileSecurityHelper.isDangerousExtension(ext)) {
                            val shareIntent = Intent(Intent.ACTION_SEND).apply {
                                type = mime ?: "application/octet-stream"
                                putExtra(Intent.EXTRA_STREAM, uri)
                                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                            }
                            val chooser = Intent.createChooser(
                                shareIntent,
                                ctx.getString(R.string.file_open_dangerous_ext),
                            ).apply { addFlags(Intent.FLAG_ACTIVITY_NEW_TASK) }
                            ctx.startActivity(chooser)
                        } else {
                            val intent = Intent(Intent.ACTION_VIEW).apply {
                                setDataAndType(uri, mime ?: "*/*")
                                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                            }
                            if (ctx.packageManager.resolveActivity(intent, PackageManager.MATCH_DEFAULT_ONLY) != null) {
                                ctx.startActivity(intent)
                            } else {
                                toastState.show(ctx.getString(R.string.file_open_no_app), GlassToastKind.DANGER)
                            }
                        }
                    } else {
                        toastState.show(safeNameOrError, GlassToastKind.DANGER)
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

// ─────────────────────────────────────────────────────────────────────────────
// List
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun HistoryList(
    items: List<ClipboardItem>,
    padding: PaddingValues,
    /** A-C1: skin tokens for row treatment (CARD/LINE/INSET) and row gap (INSET only). */
    tok: SkinTokens,
    selectionMode: Boolean,
    selectedIds: Set<String>,
    reorderMode: Boolean = false,
    hasMore: Boolean = false,
    onLoadMore: () -> Unit = {},
    ownDeviceId: String = "",
    peers: List<PairedPeer> = emptyList(),
    onDelete: (String) -> Unit,
    onSetPinned: (String, Boolean) -> Unit,
    /** Called with (itemId, direction) where direction is -1 (up) or +1 (down). */
    onReorderPinned: (String, Int) -> Unit = { _, _ -> },
    /** Called with the item id AFTER it was copied, to bump it to the top (recency). */
    onCopied: (String) -> Unit = {},
    onLongPress: (String) -> Unit,
    onCheckboxTap: (String) -> Unit,
    onSensitiveTap: () -> Unit = {},
    /** Called when the user taps Save on a file row; receives the item id. */
    onSaveFile: (String) -> Unit = {},
    /** Called when the user taps Open on a file row; receives the item id. */
    onOpenFile: (String) -> Unit = {},
    /** Called when long-press starts — shows the peek preview card. */
    onPreviewPeek: (String) -> Unit = {},
    /** Called when drag-up commits — pins the preview card. */
    onPreviewPin: (String) -> Unit = {},
    /** Called when peek is dismissed without committing. */
    onPreviewDismiss: () -> Unit = {},
    /**
     * CopyPaste-5917.76: called when paste-as-plain-text is ON and the user taps an image
     * or file row — these items have no usable plaintext payload, so the copy would silently
     * fall back to the item's snippet (e.g. "[image]"). Instead of setting a useless clip,
     * the callback is invoked with the human-readable error string so the caller can show
     * a toast. Clipboard is NOT modified when this fires.
     */
    onMediaCopyAsText: (String) -> Unit = {},
) {
    val ctx = LocalContext.current
    val settings = remember { Settings(ctx) }
    val repository = remember { ClipboardRepository(ctx) }
    val scope = rememberCoroutineScope()
    // CopyPaste-998 (jank): pull the active ramp ONCE at list scope and pass it into
    // every row, so each row body does NOT touch the CompositionLocal during scroll
    // recomposition. LocalIdeColors is staticCompositionLocalOf (changes only on a
    // full theme switch / activity recreate), so a single read here is stable for
    // the list's lifetime.
    val c = LocalIdeColors.current
    // §8 a11y: skip animated transitions when the user has requested reduced motion.
    val reducedMotion = rememberReducedMotion()
    // E: hoist settings reads via a version token so they're re-read once per
    // settings-change event rather than on every recomposition frame.
    // A DisposableEffect observes the settings SharedPreferences and increments
    // settingsVersion whenever any key changes; the three remember(settingsVersion)
    // blocks re-run only on that tick, not on list-scroll recompositions.
    var settingsVersion by remember { androidx.compose.runtime.mutableIntStateOf(0) }
    androidx.compose.runtime.DisposableEffect(ctx) {
        val listener = android.content.SharedPreferences.OnSharedPreferenceChangeListener { _, _ ->
            settingsVersion++
        }
        val sp = ctx.getSharedPreferences("copypaste", android.content.Context.MODE_PRIVATE)
        sp.registerOnSharedPreferenceChangeListener(listener)
        onDispose { sp.unregisterOnSharedPreferenceChangeListener(listener) }
    }
    val maskSensitive = remember(settingsVersion) { settings.maskSensitiveContent }
    val imageMaxHeightDp = remember(settingsVersion) { settings.imageMaxHeight }
    val previewDelayMs = remember(settingsVersion) { settings.previewDelay }
    // §3/P1#9: honour the preview-lines pref as the row's preview maxLines.
    val previewLines = remember(settingsVersion) { settings.previewLines }
    // §2 density-aware row height: read the same "density" key the Settings store
    // (Settings.density) writes — it persists the Density enum *name* ("COMPACT"/
    // "COMFORTABLE"), so compare case-insensitively. Default to comfortable (34dp)
    // when the key is absent. Keyed on settingsVersion so a toggle re-renders rows.
    val isCompact = remember(settingsVersion) {
        ctx.getSharedPreferences("copypaste", android.content.Context.MODE_PRIVATE)
            .getString("density", "comfortable")
            ?.equals("compact", ignoreCase = true) ?: false
    }

    // CopyPaste-5917.76: rememberUpdatedState captures the latest onMediaCopyAsText without
    // invalidating the copyItemById remember key. The lambda inside always calls the most
    // recently provided callback (stable indirection), so callers can update the lambda without
    // forcing a reallocation of copyItemById.
    val currentOnMediaCopyAsText by androidx.compose.runtime.rememberUpdatedState(onMediaCopyAsText)

    // D: hoist the per-item copy logic into a single stable lambda (copyItemById) that
    // captures only stable screen-level values (ctx, repository, settings, scope).
    // Previously the entire onCopy body was freshly allocated per row per recomposition,
    // capturing `item` (a different object each time). Now every row shares the same
    // function object; only the item is passed as a parameter at call time.
    val copyItemById: (ClipboardItem) -> Unit = remember(ctx, repository, scope) {
        { item ->
            scope.launch {
                val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                // CopyPaste-v0yi: read the setting at call time (settings is captured;
                // it always reflects the current persisted value). When true, all item
                // types are downgraded to plain text — the image/file URI branches are
                // skipped so the pasted content is always human-readable plain text.
                val forcePlainText = settings.pasteAsPlainText
                when {
                    item.isImage && !forcePlainText -> {
                        // Image copy-back: write full-res bytes to a cache file
                        // and expose via FileProvider so the system clipboard
                        // receives a proper content:// URI instead of "[image]".
                        val imageBytes = withContext(Dispatchers.IO) {
                            repository.getImageBytes(item.id)
                        }
                        if (imageBytes != null) {
                            val uri = withContext(Dispatchers.IO) {
                                try {
                                    val dir = File(ctx.cacheDir, "image_copy").also { it.mkdirs() }
                                    val file = File(dir, "${item.id}.png")
                                    file.writeBytes(imageBytes)
                                    FileProvider.getUriForFile(
                                        ctx,
                                        "${ctx.packageName}.fileprovider",
                                        file,
                                    )
                                } catch (e: Exception) {
                                    android.util.Log.w("HistoryActivity", "image copy-back FileProvider failed: ${e.message}")
                                    null
                                }
                            }
                            if (uri != null) {
                                val clip = ClipData.newUri(ctx.contentResolver, "CopyPaste image", uri)
                                clip.addItem(ClipData.Item(uri))
                                // CopyPaste-5917.73: narrowed grant — image/png targets only
                                // (was all-packages; now limited to clipboard/share handlers + OEM hardlist).
                                grantUriToAll(ctx, uri, "image/png")
                                // Register the expected URI BEFORE setPrimaryClip so
                                // the capture listeners recognise this as an internal
                                // copy-from-history echo and do NOT re-store it as a
                                // duplicate row (parity with the text expectClip guard).
                                ClipboardRepository.expectImageUri(uri)
                                cm.setPrimaryClip(clip)
                            }
                            // else: image bytes unavailable, nothing to copy
                        }
                    }
                    item.isFile && !forcePlainText -> {
                        // File copy-back: write bytes to a cache file and
                        // expose via FileProvider as a content:// URI.
                        val fileBytes = withContext(Dispatchers.IO) {
                            repository.getFileBytes(item.id)
                        }
                        if (fileBytes != null) {
                            val uri = withContext(Dispatchers.IO) {
                                try {
                                    val (fileName, _) = repository.getFileMeta(item.id)
                                    val safeName = fileName?.takeIf { it.isNotBlank() }
                                        ?: "${item.id}.bin"
                                    val dir = File(ctx.cacheDir, "file_copy").also { it.mkdirs() }
                                    val file = File(dir, safeName)
                                    file.writeBytes(fileBytes)
                                    FileProvider.getUriForFile(
                                        ctx,
                                        "${ctx.packageName}.fileprovider",
                                        file,
                                    )
                                } catch (e: Exception) {
                                    android.util.Log.w("HistoryActivity", "file copy-back FileProvider failed: ${e.message}")
                                    null
                                }
                            }
                            if (uri != null) {
                                val clip = ClipData.newUri(ctx.contentResolver, "CopyPaste file", uri)
                                // CopyPaste-5917.73: narrowed grant — octet-stream targets only.
                                grantUriToAll(ctx, uri, "application/octet-stream")
                                // Register the expected URI BEFORE setPrimaryClip (same
                                // guard as image copy-back above and text expectClip).
                                ClipboardRepository.expectImageUri(uri)
                                cm.setPrimaryClip(clip)
                            }
                            // else: file bytes unavailable or FileProvider failed; nothing to copy
                        }
                    }
                    else -> {
                        // CopyPaste-5917.76: when paste-as-plain-text is ON, image and file
                        // items reach this branch because their typed branches require
                        // !forcePlainText. These items have no usable plaintext payload
                        // (loadFullPlaintext returns null; snippet is "[image]" etc.).
                        // Instead of silently setting a useless clipboard entry, notify the
                        // user and leave the clipboard unchanged — matching macOS behaviour.
                        if (forcePlainText && (item.isImage || item.isFile)) {
                            currentOnMediaCopyAsText(
                                ctx.getString(R.string.error_cannot_paste_as_text)
                            )
                            return@launch  // do not update clipboard; skip onCopied bump
                        }
                        val key = settings.encryptionKey
                        val fullText = repository.loadFullPlaintext(item.id, key)
                            ?: item.snippet
                        // Register the expected content-hash BEFORE setting
                        // the clip so the capture listeners recognise this
                        // as an internal copy-from-history echo and do not
                        // re-capture it as a duplicate row + cloud re-push.
                        ClipboardRepository.expectClip(fullText)
                        cm.setPrimaryClip(ClipData.newPlainText("CopyPaste", fullText))
                    }
                }
                // Move the copied clip to the top of the recency section
                // (no-op for pinned items). Mirrors macOS bump_item_recency.
                onCopied(item.id)
            }
        }
    }

    // G: track already-mounted ids outside the LazyColumn so the remember {} is called
    // in a proper @Composable context (LazyListScope does not expose remember{}).
    // AnimatedVisibility only plays the entrance animation once per id; re-emitted rows
    // (same id) skip the animation entirely. mutableSetOf is a plain MutableSet — mutations
    // inside itemsIndexed are on the composition thread and do not need Compose state.
    @Suppress("RememberReturnType")
    val mountedIds = remember { mutableSetOf<String>() }

    val listState = rememberLazyListState()

    // Infinite scroll: trigger loadMore when within 10 items of the end and hasMore is true.
    val shouldLoadMore by remember {
        derivedStateOf {
            if (!hasMore) return@derivedStateOf false
            val layoutInfo = listState.layoutInfo
            val totalItems = layoutInfo.totalItemsCount
            if (totalItems == 0) return@derivedStateOf false
            val lastVisible = layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0
            lastVisible >= totalItems - 10
        }
    }
    LaunchedEffect(shouldLoadMore) {
        if (shouldLoadMore) onLoadMore()
    }

    // §1 aurora: let the Scaffold's aurora backdrop show through the list when
    // translucency is on. c.bg fill only in the solid (accessibility) mode.
    val listTranslucent = rememberTranslucency()
    // Hoist entrance duration once at list scope so it is NOT recomputed per row
    // inside itemsIndexed (motionDuration reads LocalLiquidTokens — stable, but
    // calling remember per-item still adds per-item composition state entries).
    val rowEnterDurMs = motionDuration(Motion.Base)
    // A-C1: row gap driven by tok.rowGap (0 for CARD/LINE, tok.rowGap for INSET).
    // Classic path: tok.rowGap=0 → Arrangement.spacedBy(0.dp) — byte-identical to pre-skin.
    val rowGap = tok.rowGap
    // A-C1: INSET treatment adds horizontal padding to inset the rows inside a recessed look.
    val isInset = tok.rowTreatment == SkinRowTreatment.INSET

    LazyColumn(
        state = listState,
        modifier = Modifier
            .fillMaxSize()
            .background(if (listTranslucent) Color.Transparent else c.bg)
            .padding(padding),
        contentPadding = PaddingValues(
            // A-C1 INSET: add top+bottom content padding equal to rowGap so the first and
            // last rows are also visually separated from the list edges. CARD/LINE: no padding.
            top = if (isInset) rowGap else 0.dp,
            bottom = if (isInset) rowGap else 0.dp,
        ),
        // A-C1: row spacing — CARD/LINE=0dp (divider-separated), INSET=tok.rowGap (card-spaced).
        // Classic: spacedBy(0.dp) — identical to previous Arrangement.spacedBy(0.dp).
        verticalArrangement = Arrangement.spacedBy(rowGap),
    ) {
        val pinnedCount = items.count { it.pinned }
        itemsIndexed(items, key = { _, item -> item.id }) { index, item ->
            // G: only animate on the first appearance of this id; subsequent re-emits
            // (same id, same data) are already mounted and should skip animation.
            val isNewMount = !mountedIds.contains(item.id)
            if (isNewMount) mountedIds.add(item.id)
            // CopyPaste-z89 (stagger): ~20ms step, cap 10 rows (was Motion.Fast=130ms,
            // i.e. up to 1.3s — far too slow). Matches PARITY-SPEC §11 (18–20ms / cap 10).
            val mountDelay = if (isNewMount)
                (index * ROW_STAGGER_STEP_MS).coerceAtMost(10 * ROW_STAGGER_STEP_MS)
            else 0
            // §8 a11y: suppress entrance animation entirely when reduced-motion is active.
            // Styleguide .listItemIn: translateX(-12px) → 0, 0.55s out-expo — horizontal
            // slide from left matches the web parity spec. rowEnterDurMs is hoisted at
            // list scope (motionDuration is @Composable — per-item call adds state entries).
            AnimatedVisibility(
                visible = true,
                enter = if (reducedMotion || !isNewMount) androidx.compose.animation.EnterTransition.None
                        else fadeIn(
                            animationSpec = tween(
                                durationMillis = rowEnterDurMs,
                                delayMillis = mountDelay,
                                easing = EaseOutExpo,
                            )
                        ) + slideInHorizontally(
                            animationSpec = tween(
                                durationMillis = rowEnterDurMs,
                                delayMillis = mountDelay,
                                easing = EaseOutExpo,
                            ),
                            // Styleguide: translateX(-12px) — small left-offset entrance.
                            initialOffsetX = { -it / 5 },
                        ),
            ) {
                // A-C1: INSET rows wrap in a horizontally-inset Column with rounded corners
                // (Vapor inset card look). CARD/LINE rows use the flat Column (byte-identical).
                Column(
                    modifier = Modifier
                        .previewPeekGesture(
                            itemId = item.id,
                            selectionMode = selectionMode,
                            onPeeking = onPreviewPeek,
                            onPinned = onPreviewPin,
                            onDismissPeek = onPreviewDismiss,
                        )
                        .then(
                            // INSET: add horizontal inset margin + rounded card background.
                            // The radius matches tok.radiusCard (Vapor=16dp) for visual harmony.
                            // CARD/LINE: no extra modifier — preserves byte-identical Classic look.
                            //
                            // Q7: background alpha derived from tok.fillAlpha so it tracks the
                            // skin's surface-fill opacity (fillAlpha * 0.76 ≈ 0.38 for Vapor 0.50).
                            // Q8: horizontal inset = tok.rowGap × (8/3) so the gap between
                            // the card edge and the list rail scales with the skin's row rhythm
                            // (Vapor rowGap=3dp → 8dp, future skins auto-scale).
                            if (isInset) Modifier
                                .padding(horizontal = tok.rowGap * (8f / 3f), vertical = 0.dp)
                                .background(
                                    color = c.elevated.copy(alpha = tok.fillAlpha * 0.76f),
                                    shape = RoundedCornerShape(tok.radiusCard),
                                )
                            else Modifier
                        ),
                ) {
                    HistoryRow(
                        item = item,
                        colors = c,
                        repository = repository,
                        maskSensitive = maskSensitive,
                        imageMaxHeightDp = imageMaxHeightDp,
                        previewDelayMs = previewDelayMs,
                        previewLines = previewLines,
                        isCompact = isCompact,
                        selectionMode = selectionMode,
                        isSelected = selectedIds.contains(item.id),
                        reorderMode = reorderMode,
                        pinnedIndex = item.pinnedSortIndex,
                        pinnedCount = pinnedCount,
                        ownDeviceId = ownDeviceId,
                        peers = peers,
                        onDelete = onDelete,
                        onSetPinned = onSetPinned,
                        onMoveUp = { onReorderPinned(item.id, -1) },
                        onMoveDown = { onReorderPinned(item.id, +1) },
                        onCopy = { copyItemById(item) },
                        onLongPress = { onLongPress(item.id) },
                        onCheckboxTap = { onCheckboxTap(item.id) },
                        onSensitiveTap = onSensitiveTap,
                        onSaveFile = { onSaveFile(item.id) },
                        onOpenFile = { onOpenFile(item.id) },
                        onPreviewPeek = onPreviewPeek,
                        onPreviewPin = onPreviewPin,
                        onPreviewDismiss = onPreviewDismiss,
                    )
                    // A-C1: divider shown for CARD and LINE treatments; suppressed for INSET
                    // (spacing between cards replaces the divider line in the Vapor skin).
                    // Classic (CARD) shows the divider — byte-identical to pre-skin behaviour.
                    if (tok.rowTreatment != SkinRowTreatment.INSET) {
                        HorizontalDivider(
                            color = c.divider,
                            thickness = 1.dp,
                        )
                    }
                }
            }
        }
        // Footer: subtle loading indicator while next page loads
        if (hasMore) {
            item(key = "__load_more_footer__") {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 12.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    CircularProgressIndicator(
                        color = c.accent.copy(alpha = 0.5f),
                        strokeWidth = 1.5.dp,
                        modifier = Modifier.size(16.dp),
                    )
                }
            }
        }
    }
}

