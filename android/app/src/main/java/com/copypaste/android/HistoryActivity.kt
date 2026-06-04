@file:OptIn(ExperimentalFoundationApi::class)

package com.copypaste.android

import android.graphics.BitmapFactory
import android.os.Bundle
import android.util.Base64
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.slideInVertically
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.BookmarkAdded
import androidx.compose.material.icons.filled.BookmarkBorder
import androidx.compose.material.icons.filled.CheckBox
import androidx.compose.material.icons.filled.CheckBoxOutlineBlank
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.CloudOff
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Image
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.SaveAlt
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.SwapVert
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.TextField
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.livedata.observeAsState
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.statusBars
import kotlinx.coroutines.launch
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.scale
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
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.ideTextFieldColors
import com.copypaste.android.ui.theme.EaseOutExpo
import com.copypaste.android.ui.theme.IdeAccent
import com.copypaste.android.ui.theme.IdeAccentDim
import com.copypaste.android.ui.theme.IdeBg
import com.copypaste.android.ui.theme.IdeBorder
import com.copypaste.android.ui.theme.IdeDanger
import com.copypaste.android.ui.theme.IdeDangerDim
import com.copypaste.android.ui.theme.IdeDim
import com.copypaste.android.ui.theme.IdeElevated
import com.copypaste.android.ui.theme.IdeFaint
import com.copypaste.android.ui.theme.IdeInfo
import com.copypaste.android.ui.theme.IdeInfoDim
import com.copypaste.android.ui.theme.IdePanel
import com.copypaste.android.ui.theme.IdeSelection
import com.copypaste.android.ui.theme.IdeText
import com.copypaste.android.ui.theme.IdeViolet
import com.copypaste.android.ui.theme.IdeVioletDim
import com.copypaste.android.ui.theme.IdeWarning
import com.copypaste.android.ui.theme.IdeWarningDim
import com.copypaste.android.ui.theme.Motion
import kotlinx.coroutines.delay
import java.text.DateFormat
import java.util.Date

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
        enableEdgeToEdge()
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
// Confirmation dialog enum
// ─────────────────────────────────────────────────────────────────────────────

private enum class ConfirmAction { CLEAR_UNPINNED, DELETE_SELECTED }

// ─────────────────────────────────────────────────────────────────────────────
// Relative time helper — §5 tabular-nums timestamps
// ─────────────────────────────────────────────────────────────────────────────

private fun relativeTime(ms: Long): String {
    if (ms <= 0L) return "—"
    val diff = System.currentTimeMillis() - ms
    return when {
        diff < 60_000L      -> "just now"
        diff < 3_600_000L   -> "${diff / 60_000}m ago"
        diff < 86_400_000L  -> "${diff / 3_600_000}h ago"
        diff < 7 * 86_400_000L -> "${diff / 86_400_000}d ago"
        else -> DateFormat.getDateInstance(DateFormat.SHORT).format(Date(ms))
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
) {
    val items by viewModel.items.observeAsState(emptyList())
    val loading by viewModel.loading.observeAsState(false)
    val error by viewModel.errors.observeAsState(null)
    val totalCount by viewModel.totalCount.observeAsState(0)
    val hasMore by viewModel.hasMore.observeAsState(false)
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val ctx = LocalContext.current
    val settings = remember { Settings(ctx) }
    val loadErrorTemplate = stringResource(R.string.error_load_history)
    val dismissLabel = stringResource(R.string.snackbar_dismiss)
    val sensitiveTapMsg = stringResource(R.string.sensitive_tap_hint)

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

    // ── Long-press peek preview state ─────────────────────────────────────────
    // previewItemId + previewPhase survive rotation via rememberSaveable.
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

    // Auto-dismiss preview when item leaves the list
    LaunchedEffect(items, previewItemId) {
        val id = previewItemId ?: return@LaunchedEffect
        if (items.none { it.id == id }) {
            previewItemId = null
            previewPhase = PreviewPhase.Idle
        }
    }

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
            restore = { ord -> ord?.let { ConfirmAction.entries[it] } },
        )
    ) { mutableStateOf<ConfirmAction?>(null) }
    var overflowExpanded by rememberSaveable { mutableStateOf(false) }

    // ── Reorder mode (pinned items only) ────────────────────────────────────
    var reorderMode by rememberSaveable { mutableStateOf(false) }

    BackHandler(enabled = reorderMode) { reorderMode = false }

    // Sort: pinned first (by user-defined pinnedSortIndex), then unpinned by recency.
    // Pinned items are sorted by pinnedSortIndex (NOT wallTimeMs) so copying a pinned
    // clip does not move it — fixes HW-A15.
    val sortedItems = remember(items) {
        // Defensive de-dup by id BEFORE the list reaches the LazyColumn. The list
        // backing the LazyColumn uses `key = { it.id }`, so a duplicate id throws
        // IllegalArgumentException ("Key … was already used") and crash-loops the
        // screen. A persistent duplicate can arise in the repository id index (e.g.
        // a synced item re-appended under the same overrideId after the
        // synced-source-id seen-set was cleared by clearUnpinned). Collapsing
        // duplicates here guarantees the LazyColumn can never crash regardless of
        // how the backing store drifts; the repository fix below removes the source.
        items.distinctBy { it.id }
            .sortedWith(
                compareByDescending<ClipboardItem> { it.pinned }
                    .thenBy { if (it.pinned) it.pinnedSortIndex else 0 }
                    .thenByDescending { it.wallTimeMs }
            )
    }
    // Filter: case-insensitive substring match on snippet
    val filteredItems = remember(sortedItems, searchQuery) {
        val q = searchQuery.trim()
        if (q.isEmpty()) sortedItems
        else sortedItems.filter { it.snippet.contains(q, ignoreCase = true) }
    }

    // ── Device filter (parity with macOS) ────────────────────────────────────
    // Collect distinct origin device ids from the FULL sorted list (not search-
    // filtered) so the filter chips are stable while typing. Show the chips only
    // when more than one device is present — mirrors macOS HistoryView.
    val originDeviceIds = remember(sortedItems) { distinctOriginDeviceIds(sortedItems) }
    val ownDeviceId = remember { settings.deviceId }
    val pairedPeers = remember { settings.pairedPeers }

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
        snackbarHostState.showSnackbar(
            message = loadErrorTemplate.format(msg),
            actionLabel = dismissLabel,
        )
        viewModel.clearError()
    }

    // ── Confirmation dialog ──────────────────────────────────────────────────
    pendingConfirm?.let { action ->
        ConfirmationDialog(
            action = action,
            itemCount = when (action) {
                ConfirmAction.CLEAR_UNPINNED -> items.count { !it.pinned }
                ConfirmAction.DELETE_SELECTED -> selectedIds.size
            },
            onConfirm = {
                pendingConfirm = null
                when (action) {
                    ConfirmAction.CLEAR_UNPINNED -> viewModel.clearUnpinned()
                    ConfirmAction.DELETE_SELECTED -> {
                        viewModel.deleteItems(selectedIds.toList())
                        selectionMode = false
                        selectedIds = emptySet()
                    }
                }
            },
            onDismiss = { pendingConfirm = null },
        )
    }

    Scaffold(
        modifier = modifier,
        containerColor = IdeBg,
        topBar = {
            if (selectionMode) {
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
                )
            } else {
                // HW-A8 / search-overlay fix: the recent-searches list used to be a
                // Popup (DropdownMenu) anchored to the narrow actions Box, so it
                // overlaid and blocked the history list and never dismissed. It is
                // now an INLINE full-width search Row + suggestions Column rendered
                // in the topBar Column, so it pushes content down via innerPadding
                // instead of floating over it.
                val searchFocusRequester = remember { FocusRequester() }
                val keyboardController = LocalSoftwareKeyboardController.current
                val clearRecentLabel = stringResource(R.string.action_clear_recent_searches)

                Column(modifier = Modifier.background(IdePanel)) {
                    TopAppBar(
                        title = {
                            // Show the full stored total (not just loaded count) next to
                            // the title — macOS parity for the history header item count.
                            val countLabel = if (totalCount > 0) " ($totalCount)" else ""
                            Text(
                                text = stringResource(R.string.title_history) + countLabel,
                                style = MaterialTheme.typography.titleLarge,
                                color = IdeText,
                            )
                        },
                        navigationIcon = {
                            if (showBackButton) {
                                IconButton(onClick = onBack) {
                                    Icon(
                                        Icons.AutoMirrored.Filled.ArrowBack,
                                        contentDescription = stringResource(R.string.cd_back),
                                        tint = IdeDim,
                                        modifier = Modifier.size(18.dp),
                                    )
                                }
                            }
                        },
                        actions = {
                            // Search toggle icon — toggles the inline full-width search Row below.
                            IconButton(onClick = { searchExpanded = !searchExpanded }) {
                                Icon(
                                    if (searchExpanded) Icons.Filled.Close else Icons.Filled.Search,
                                    contentDescription = stringResource(
                                        if (searchExpanded) R.string.cd_search_close
                                        else R.string.cd_search_open
                                    ),
                                    tint = if (searchExpanded) IdeAccent else IdeDim,
                                    modifier = Modifier.size(18.dp),
                                )
                            }
                            IconButton(onClick = { viewModel.loadItems() }) {
                                Icon(
                                    Icons.Filled.Refresh,
                                    contentDescription = stringResource(R.string.cd_refresh),
                                    tint = IdeDim,
                                    modifier = Modifier.size(18.dp),
                                )
                            }
                            // Reorder toggle — only shown when there are ≥2 pinned items
                            val pinnedCount = items.count { it.pinned }
                            if (pinnedCount >= 2) {
                                IconButton(onClick = { reorderMode = !reorderMode }) {
                                    Icon(
                                        Icons.Filled.SwapVert,
                                        contentDescription = stringResource(R.string.cd_reorder_handle),
                                        tint = if (reorderMode) IdeAccent else IdeDim,
                                        modifier = Modifier.size(18.dp),
                                    )
                                }
                            }
                            if (items.isNotEmpty()) {
                                Box {
                                    IconButton(onClick = { overflowExpanded = true }) {
                                        Icon(
                                            Icons.Filled.MoreVert,
                                            contentDescription = null,
                                            tint = IdeDim,
                                            modifier = Modifier.size(18.dp),
                                        )
                                    }
                                    DropdownMenu(
                                        expanded = overflowExpanded,
                                        onDismissRequest = { overflowExpanded = false },
                                    ) {
                                        val unpinnedCount = items.count { !it.pinned }
                                        if (unpinnedCount > 0) {
                                            DropdownMenuItem(
                                                text = {
                                                    Text(
                                                        stringResource(R.string.action_clear_unpinned),
                                                        color = IdeText,
                                                    )
                                                },
                                                leadingIcon = {
                                                    Icon(Icons.Filled.Delete, null, tint = IdeDim)
                                                },
                                                onClick = {
                                                    overflowExpanded = false
                                                    pendingConfirm = ConfirmAction.CLEAR_UNPINNED
                                                },
                                            )
                                        }
                                    }
                                }
                            }
                        },
                        colors = TopAppBarDefaults.topAppBarColors(
                            containerColor             = IdePanel,
                            titleContentColor          = IdeText,
                            actionIconContentColor     = IdeDim,
                            navigationIconContentColor = IdeDim,
                        ),
                        windowInsets = TopAppBarDefaults.windowInsets,
                    )

                    // Full-width inline search field + suggestions, in normal layout
                    // flow (NOT a Popup) so they push the list down via innerPadding.
                    AnimatedVisibility(
                        visible = searchExpanded,
                        enter = expandVertically() + fadeIn(),
                        exit = shrinkVertically() + fadeOut(),
                    ) {
                        Column(modifier = Modifier.fillMaxWidth()) {
                            TextField(
                                value = searchQuery,
                                onValueChange = { searchQuery = it },
                                placeholder = {
                                    Text(
                                        text = stringResource(R.string.history_search_placeholder),
                                        style = MaterialTheme.typography.bodyMedium,
                                        color = IdeFaint,
                                    )
                                },
                                singleLine = true,
                                colors = ideTextFieldColors(),
                                textStyle = MaterialTheme.typography.bodyMedium.copy(color = IdeText),
                                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                                keyboardActions = KeyboardActions(onSearch = {
                                    val q = searchQuery.trim()
                                    if (q.isNotEmpty()) {
                                        // Persist to recent-5, dedup, newest first.
                                        val updated = (listOf(q) + recentSearches.filter { it != q })
                                            .take(5)
                                        recentSearches = updated
                                        settings.recentSearches = updated
                                    }
                                    keyboardController?.hide()
                                }),
                                leadingIcon = {
                                    Icon(
                                        Icons.Filled.Search, null,
                                        tint = IdeDim, modifier = Modifier.size(16.dp),
                                    )
                                },
                                trailingIcon = {
                                    if (searchQuery.isNotEmpty()) {
                                        IconButton(onClick = { searchQuery = "" }) {
                                            Icon(
                                                Icons.Filled.Close,
                                                contentDescription = stringResource(R.string.cd_search_close),
                                                tint = IdeDim,
                                                modifier = Modifier.size(16.dp),
                                            )
                                        }
                                    }
                                },
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .focusRequester(searchFocusRequester),
                            )
                            // Inline recent-searches list — full width, in flow.
                            if (searchQuery.isEmpty() && recentSearches.isNotEmpty()) {
                                Row(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .padding(horizontal = 12.dp, vertical = 4.dp),
                                    horizontalArrangement = Arrangement.SpaceBetween,
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    Text(
                                        text = stringResource(R.string.history_recent_searches),
                                        style = MaterialTheme.typography.labelSmall,
                                        color = IdeFaint,
                                    )
                                    Text(
                                        text = clearRecentLabel,
                                        style = MaterialTheme.typography.labelSmall,
                                        color = IdeAccent,
                                        modifier = Modifier.clickable {
                                            recentSearches = emptyList()
                                            settings.recentSearches = emptyList()
                                        },
                                    )
                                }
                                recentSearches.forEach { recent ->
                                    Row(
                                        modifier = Modifier
                                            .fillMaxWidth()
                                            .clickable {
                                                searchQuery = recent
                                                keyboardController?.hide()
                                            }
                                            .padding(horizontal = 12.dp, vertical = 10.dp),
                                        verticalAlignment = Alignment.CenterVertically,
                                    ) {
                                        Icon(
                                            Icons.Filled.Search, null,
                                            tint = IdeDim, modifier = Modifier.size(14.dp),
                                        )
                                        Spacer(modifier = Modifier.width(10.dp))
                                        Text(
                                            recent,
                                            color = IdeText,
                                            style = MaterialTheme.typography.bodyMedium,
                                        )
                                    }
                                }
                            }
                        }
                    }
                }

                // Request keyboard focus once search bar becomes visible.
                LaunchedEffect(searchExpanded) {
                    if (searchExpanded) {
                        searchFocusRequester.requestFocus()
                    } else {
                        keyboardController?.hide()
                    }
                }

                // ── Device filter chips — shown only when > 1 origin device present ──
                // Mirrors macOS HistoryView: filter strip is hidden for a single device.
                if (originDeviceIds.size > 1) {
                    DeviceFilterRow(
                        deviceIds = originDeviceIds,
                        selected = deviceFilter,
                        ownDeviceId = ownDeviceId,
                        peers = pairedPeers,
                        onSelect = { deviceFilter = it },
                    )
                }
            }
        },
        snackbarHost = { SnackbarHost(hostState = snackbarHostState) },
    ) { innerPadding ->
        // The overlay must be a sibling of the list inside this Box so the long-press
        // drag gesture remains one continuous pointer stream (not broken by a Dialog/Popup
        // window boundary). The overlay uses WindowInsets.statusBars top padding to ensure
        // the preview card is never occluded by the status bar or app header.
        Box(modifier = Modifier.fillMaxSize()) {
        when {
            loading && sortedItems.isEmpty() -> LoadingBox(innerPadding)
            // §9: history completely empty
            sortedItems.isEmpty() -> EmptyHistoryState(innerPadding)
            // §9: search returned no results (counting device filter too)
            deviceFilteredItems.isEmpty() -> EmptySearchState(innerPadding, searchQuery.trim())
            else -> HistoryList(
                items = deviceFilteredItems,
                padding = innerPadding,
                hasMore = hasMore,
                onLoadMore = { viewModel.loadMore() },
                ownDeviceId = ownDeviceId,
                peers = pairedPeers,
                selectionMode = selectionMode,
                selectedIds = selectedIds,
                reorderMode = reorderMode,
                onDelete = { id -> viewModel.deleteItem(id) },
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
                    // Long-press enters selection mode when preview is not active
                    selectionMode = true
                    selectedIds = setOf(id)
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
                    scope.launch { snackbarHostState.showSnackbar(sensitiveTapMsg) }
                },
                onSaveFile = { id ->
                    scope.launch {
                        val repository = ClipboardRepository(ctx)
                        val saved = withContext(Dispatchers.IO) {
                            try {
                                val fileBytes = repository.getFileBytes(id) ?: return@withContext false
                                val (fileName, mime) = repository.getFileMeta(id)
                                val safeName = fileName?.takeIf { it.isNotBlank() } ?: "file_$id.bin"
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
                        snackbarHostState.showSnackbar(
                            if (saved) ctx.getString(R.string.file_saved_ok)
                            else ctx.getString(R.string.file_save_failed)
                        )
                    }
                },
            )
        }

        // ── Preview overlay — in-tree sibling of the list, never a Dialog/Popup ──
        // The overlay applies WindowInsets.statusBars top padding to ensure the card
        // is never occluded by the status bar or app header on any device.
        val previewItem = remember(previewItemId, sortedItems) {
            previewItemId?.let { id -> sortedItems.find { it.id == id } }
        }
        val previewRepository = remember { ClipboardRepository(ctx) }
        PreviewOverlay(
            phase = previewPhase,
            item = previewItem,
            repository = previewRepository,
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
                            val imageBytes = withContext(Dispatchers.IO) { previewRepository.getImageBytes(item.id) }
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
                                    ctx.grantUriPermission("com.android.systemui", uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
                                    cm.setPrimaryClip(clip)
                                }
                            }
                        }
                        item.isFile -> {
                            val fileBytes = withContext(Dispatchers.IO) { previewRepository.getFileBytes(item.id) }
                            if (fileBytes != null) {
                                val uri = withContext(Dispatchers.IO) {
                                    try {
                                        val (fileName, _) = previewRepository.getFileMeta(item.id)
                                        val safeName = fileName?.takeIf { it.isNotBlank() } ?: "${item.id}.bin"
                                        val dir = File(ctx.cacheDir, "file_copy").also { it.mkdirs() }
                                        val file = File(dir, safeName)
                                        file.writeBytes(fileBytes)
                                        FileProvider.getUriForFile(ctx, "${ctx.packageName}.fileprovider", file)
                                    } catch (_: Exception) { null }
                                }
                                if (uri != null) {
                                    val clip = ClipData.newUri(ctx.contentResolver, "CopyPaste file", uri)
                                    ctx.grantUriPermission("com.android.systemui", uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
                                    cm.setPrimaryClip(clip)
                                }
                            }
                        }
                        else -> {
                            val fullText = withContext(Dispatchers.IO) {
                                previewRepository.loadFullPlaintext(item.id, settings.encryptionKey)
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
                val id = previewItemId ?: return@PreviewOverlay
                previewItemId = null
                previewPhase = PreviewPhase.Idle
                viewModel.deleteItem(id)
            },
            onSaveFile = {
                val id = previewItemId ?: return@PreviewOverlay
                scope.launch {
                    val repository = ClipboardRepository(ctx)
                    val saved = withContext(Dispatchers.IO) {
                        try {
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
                    snackbarHostState.showSnackbar(
                        if (saved) ctx.getString(R.string.file_saved_ok)
                        else ctx.getString(R.string.file_save_failed)
                    )
                }
            },
        )
        } // end Box
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Contextual selection top bar — §5 neutral (not amber), E2 elevation
// ─────────────────────────────────────────────────────────────────────────────

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SelectionTopBar(
    selectedCount: Int,
    totalCount: Int,
    onClose: () -> Unit,
    onSelectAll: () -> Unit,
    onDeleteSelected: () -> Unit,
    onPinSelected: () -> Unit,
    onUnpinSelected: () -> Unit,
) {
    // §5: NEUTRAL (not amber) multi-select bar — IdeElevated container, not warning
    TopAppBar(
        title = {
            Text(
                text = stringResource(R.string.selection_count, selectedCount),
                style = MaterialTheme.typography.titleLarge,
                color = IdeText,
            )
        },
        navigationIcon = {
            IconButton(onClick = onClose) {
                Icon(
                    Icons.Filled.Close,
                    contentDescription = stringResource(R.string.cd_close_selection),
                    tint = IdeDim,
                    modifier = Modifier.size(18.dp),
                )
            }
        },
        actions = {
            val allSelected = selectedCount == totalCount && totalCount > 0
            IconButton(onClick = onSelectAll) {
                Icon(
                    if (allSelected) Icons.Filled.CheckBox else Icons.Filled.CheckBoxOutlineBlank,
                    contentDescription = stringResource(R.string.cd_select_all),
                    tint = if (allSelected) IdeAccent else IdeDim,
                    modifier = Modifier.size(18.dp),
                )
            }
            if (selectedCount > 0) {
                IconButton(onClick = onPinSelected) {
                    Icon(
                        Icons.Filled.BookmarkAdded,
                        contentDescription = stringResource(R.string.action_pin_selected),
                        tint = IdeAccent,
                        modifier = Modifier.size(18.dp),
                    )
                }
                IconButton(onClick = onUnpinSelected) {
                    Icon(
                        Icons.Filled.BookmarkBorder,
                        contentDescription = stringResource(R.string.action_unpin_selected),
                        tint = IdeDim,
                        modifier = Modifier.size(18.dp),
                    )
                }
                IconButton(onClick = onDeleteSelected) {
                    Icon(
                        Icons.Filled.Delete,
                        contentDescription = stringResource(R.string.action_delete_selected),
                        tint = IdeDanger,
                        modifier = Modifier.size(18.dp),
                    )
                }
            }
        },
        // §5 Neutral elevated container — NOT amber/warning (desktop parity)
        colors = TopAppBarDefaults.topAppBarColors(
            containerColor             = IdeElevated,
            titleContentColor          = IdeText,
            actionIconContentColor     = IdeDim,
            navigationIconContentColor = IdeDim,
        ),
        windowInsets = TopAppBarDefaults.windowInsets,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Confirmation dialog
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun ConfirmationDialog(
    action: ConfirmAction,
    itemCount: Int,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
) {
    val title = when (action) {
        ConfirmAction.CLEAR_UNPINNED -> stringResource(R.string.dialog_clear_unpinned_title)
        ConfirmAction.DELETE_SELECTED -> stringResource(R.string.dialog_delete_selected_title)
    }
    val message = when (action) {
        ConfirmAction.CLEAR_UNPINNED ->
            stringResource(R.string.dialog_clear_unpinned_message)
        ConfirmAction.DELETE_SELECTED ->
            stringResource(R.string.dialog_delete_selected_message, itemCount)
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title, color = IdeText) },
        text = { Text(message, color = IdeDim) },
        confirmButton = {
            TextButton(onClick = onConfirm) {
                Text(stringResource(R.string.dialog_confirm), color = IdeDanger)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.dialog_cancel), color = IdeDim)
            }
        },
        containerColor = IdePanel,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Loading state
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun LoadingBox(padding: PaddingValues) {
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(IdeBg)
            .padding(padding),
        contentAlignment = Alignment.Center,
    ) {
        CircularProgressIndicator(
            color = IdeAccent,
            strokeWidth = 2.dp,
            modifier = Modifier.size(20.dp),
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §9 Empty states — hero icon (28dp) + title (13sp dim) + sentence (11sp faint)
// Matches desktop HistoryView empty pattern exactly.
// ─────────────────────────────────────────────────────────────────────────────

/** §9 Empty state: history is empty — clipboard icon + "Nothing copied yet". */
@Composable
private fun EmptyHistoryState(padding: PaddingValues) {
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(IdeBg)
            .padding(padding)
            .padding(24.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            // §9 hero: clipboard icon 28dp faint (never accent)
            Icon(
                imageVector = Icons.Filled.ContentCopy,
                contentDescription = null,
                tint = IdeFaint,
                modifier = Modifier.size(28.dp),
            )
            Text(
                text = stringResource(R.string.empty_history),
                style = MaterialTheme.typography.bodyLarge,
                color = IdeDim,
            )
            Text(
                text = stringResource(R.string.empty_history_subtitle),
                style = MaterialTheme.typography.bodyMedium,
                color = IdeFaint,
            )
        }
    }
}

/** §9 Empty state: search returned no results. */
@Composable
private fun EmptySearchState(padding: PaddingValues, query: String) {
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(IdeBg)
            .padding(padding)
            .padding(24.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            // §9 hero: search icon 28dp faint
            Icon(
                imageVector = Icons.Filled.Refresh, // reuse as "search-x" visual; distinct from loading spinner
                contentDescription = null,
                tint = IdeFaint,
                modifier = Modifier.size(28.dp),
            )
            Text(
                text = stringResource(R.string.empty_search_title, query),
                style = MaterialTheme.typography.bodyLarge,
                color = IdeDim,
            )
            Text(
                text = stringResource(R.string.empty_search_subtitle),
                style = MaterialTheme.typography.bodyMedium,
                color = IdeFaint,
            )
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §3 Content-type chip — tinted pill matching desktop §4 chip anatomy
// text=accent, url=info, image=violet, code=violet, sensitive=danger
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun ContentTypeChip(contentType: String, isSensitive: Boolean) {
    val (label, fg, bg) = when {
        isSensitive -> Triple("PRIVATE", IdeDanger, IdeDangerDim)
        contentType.startsWith("image/") || contentType == "image" ->
            Triple("IMG", IdeViolet, IdeVioletDim)
        contentType == "url" || contentType.startsWith("url") ->
            Triple("URL", IdeInfo, IdeInfoDim)
        contentType == "text" || contentType.startsWith("text/") ->
            Triple("TEXT", IdeAccent, IdeAccentDim)
        else -> Triple("FILE", IdeDim, IdeElevated)
    }

    Box(
        modifier = Modifier
            .background(color = bg, shape = RoundedCornerShape(4.dp))
            .padding(horizontal = 5.dp, vertical = 2.dp),
    ) {
        Text(
            text = label,
            style = TextStyle(
                fontSize = 9.sp,
                fontWeight = FontWeight.SemiBold,
                letterSpacing = 0.4.sp,
                // fontFeatureSettings not available as direct TextStyle param in Compose 1.x;
                // tabular-nums applied via fontVariantNumeric is not directly supported in
                // Compose 1.5 either — the Compose approach is PlatformTextStyle on API 26+.
                // For now the chip label is short enough (3-7 chars) that tnum is irrelevant.
            ),
            color = fg,
            maxLines = 1,
        )
    }
}

/**
 * Small warning-tinted indicator shown on a row whose payload exceeds the sync size
 * cap ([ClipboardRepository.SYNC_MAX_BLOB_BYTES], 8 MiB) and therefore will not be
 * propagated to other devices. Sized (12.dp) and tinted ([IdeWarning]) to match the
 * adjacent pin indicator. Caller is responsible for the `!selectionMode` gating.
 */
@Composable
private fun TooLargeBadge() {
    Spacer(Modifier.width(4.dp))
    Icon(
        imageVector = Icons.Filled.CloudOff,
        contentDescription = stringResource(R.string.cd_too_large_sync),
        tint = IdeWarning.copy(alpha = 0.9f),
        modifier = Modifier.size(12.dp),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// List
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun HistoryList(
    items: List<ClipboardItem>,
    padding: PaddingValues,
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
    /** Called when long-press starts — shows the peek preview card. */
    onPreviewPeek: (String) -> Unit = {},
    /** Called when drag-up commits — pins the preview card. */
    onPreviewPin: (String) -> Unit = {},
    /** Called when peek is dismissed without committing. */
    onPreviewDismiss: () -> Unit = {},
) {
    val ctx = LocalContext.current
    val settings = remember { Settings(ctx) }
    // Read live on every recomposition so changes in SettingsActivity are reflected
    // immediately without restarting the screen (fix P2: stale remember wrappers removed).
    val maskSensitive = settings.maskSensitiveContent
    val imageMaxHeightDp = settings.imageMaxHeight
    val previewDelayMs = settings.previewDelay
    val repository = remember { ClipboardRepository(ctx) }
    val scope = rememberCoroutineScope()

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

    LazyColumn(
        state = listState,
        modifier = Modifier
            .fillMaxSize()
            .background(IdeBg)
            .padding(padding),
        contentPadding = PaddingValues(0.dp),
        verticalArrangement = Arrangement.spacedBy(0.dp),
    ) {
        // §8 mount fade/rise stagger — AnimatedVisibility per item, capped at 10 items
        // to avoid stagger on large existing lists (only on initial appearance).
        val pinnedCount = items.count { it.pinned }
        itemsIndexed(items, key = { _, item -> item.id }) { index, item ->
            val mountDelay = (index * Motion.Fast).coerceAtMost(10 * Motion.Fast)
            AnimatedVisibility(
                visible = true,
                enter = fadeIn(
                    animationSpec = tween(
                        durationMillis = Motion.Base,
                        delayMillis = mountDelay,
                        easing = EaseOutExpo,
                    )
                ) + slideInVertically(
                    animationSpec = tween(
                        durationMillis = Motion.Base,
                        delayMillis = mountDelay,
                        easing = EaseOutExpo,
                    ),
                    initialOffsetY = { it / 8 },
                ),
            ) {
                Column(
                    modifier = Modifier.previewPeekGesture(
                        itemId = item.id,
                        selectionMode = selectionMode,
                        onPeeking = onPreviewPeek,
                        onPinned = onPreviewPin,
                        onDismissPeek = onPreviewDismiss,
                    ),
                ) {
                    HistoryRow(
                        item = item,
                        maskSensitive = maskSensitive,
                        imageMaxHeightDp = imageMaxHeightDp,
                        previewDelayMs = previewDelayMs,
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
                        onCopy = {
                            scope.launch {
                                val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                                when {
                                    item.isImage -> {
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
                                                // Grant read permission so the receiving app can read the URI.
                                                ctx.grantUriPermission(
                                                    "com.android.systemui",
                                                    uri,
                                                    Intent.FLAG_GRANT_READ_URI_PERMISSION,
                                                )
                                                cm.setPrimaryClip(clip)
                                            }
                                            // else: image bytes unavailable, nothing to copy
                                        }
                                    }
                                    item.isFile -> {
                                        // File copy-back: write bytes to a cache file and
                                        // expose via FileProvider as a content:// URI.
                                        val fileBytes = withContext(Dispatchers.IO) {
                                            repository.getFileBytes(item.id)
                                        }
                                        if (fileBytes != null) {
                                            val uri = withContext(Dispatchers.IO) {
                                                try {
                                                    val (fileName, mime) = repository.getFileMeta(item.id)
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
                                                ctx.grantUriPermission(
                                                    "com.android.systemui",
                                                    uri,
                                                    Intent.FLAG_GRANT_READ_URI_PERMISSION,
                                                )
                                                cm.setPrimaryClip(clip)
                                            }
                                            // else: file bytes unavailable or FileProvider failed; nothing to copy
                                        }
                                    }
                                    else -> {
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
                        },
                        onLongPress = { onLongPress(item.id) },
                        onCheckboxTap = { onCheckboxTap(item.id) },
                        onSensitiveTap = onSensitiveTap,
                        onSaveFile = { onSaveFile(item.id) },
                    )
                    HorizontalDivider(
                        color = IdeBorder.copy(alpha = 0.5f),
                        thickness = 0.5.dp,
                    )
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
                        color = IdeAccent.copy(alpha = 0.5f),
                        strokeWidth = 1.5.dp,
                        modifier = Modifier.size(16.dp),
                    )
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Row — §5 desktop anatomy
//
// Layout (left→right):
//   [checkbox 16dp] [pin-badge?] [content-type chip] [preview text] [source-app] [timestamp] [icon-actions]
//
// §8 press-scale 0.98 via animateFloatAsState + MutableInteractionSource.
// §5 timestamp always visible (tabular-nums via fontFeatureSettings on TextStyle).
// §5 comfortable density: min height 40dp for text rows.
// ─────────────────────────────────────────────────────────────────────────────

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun HistoryRow(
    item: ClipboardItem,
    maskSensitive: Boolean,
    imageMaxHeightDp: Int,
    previewDelayMs: Long,
    selectionMode: Boolean,
    isSelected: Boolean,
    reorderMode: Boolean = false,
    pinnedIndex: Int = -1,
    pinnedCount: Int = 0,
    ownDeviceId: String = "",
    peers: List<PairedPeer> = emptyList(),
    onDelete: (String) -> Unit,
    onSetPinned: (String, Boolean) -> Unit,
    onMoveUp: () -> Unit = {},
    onMoveDown: () -> Unit = {},
    onCopy: () -> Unit = {},
    onLongPress: () -> Unit,
    onCheckboxTap: () -> Unit,
    onSensitiveTap: () -> Unit = {},
    onSaveFile: () -> Unit = {},
) {
    val detectedSensitive = item.isSensitive

    var expanded by remember(item.id) { mutableStateOf(false) }
    // Key on (item.id, expanded) so the coroutine is cancelled and restarted whenever
    // the item is rebound to a different id, preventing stale `expanded = false` writes
    // from a previous item's timer leaking into the new item (fix P1).
    LaunchedEffect(item.id, expanded) {
        if (expanded) {
            delay(previewDelayMs)
            expanded = false
        }
    }
    LaunchedEffect(selectionMode) {
        if (selectionMode) expanded = false
    }

    // §8 press-scale: 0.98 on press, instant out-expo spring back
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()
    val rowScale by animateFloatAsState(
        targetValue = if (isPressed) 0.98f else 1.0f,
        animationSpec = tween(durationMillis = Motion.Instant, easing = EaseOutExpo),
        label = "rowPressScale",
    )

    // Decode image bytes off the main thread to avoid jank.
    val imageBitmap by produceState<androidx.compose.ui.graphics.ImageBitmap?>(
        initialValue = null,
        key1 = item.id,
        key2 = item.imagePng,
    ) {
        value = item.imagePng?.let { bytes ->
            withContext(Dispatchers.Default) {
                runCatching {
                    BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.asImageBitmap()
                }.getOrElse { t ->
                    // Fix P2: log decode failures so they are diagnosable via adb/log export.
                    AppLogger.w("HistoryRow", "image decode failed for item ${item.id}", t)
                    null
                }
            }
        }
    }

    val maskString = stringResource(R.string.sensitive_preview_mask)
    val display = when {
        detectedSensitive && maskSensitive -> maskString
        item.snippet.isBlank() -> stringResource(R.string.empty_history)
        else -> item.snippet
    }

    // §5 row background: selection > expanded > sensitive tint > transparent
    val rowBg = when {
        isSelected        -> IdeSelection
        expanded          -> IdeElevated
        detectedSensitive -> IdeDanger.copy(alpha = 0.07f)
        item.pinned       -> IdeWarning.copy(alpha = 0.16f)
        else              -> Color.Transparent
    }

    // Left accent bar color: visible amber when pinned and no stronger state is active.
    val pinnedAccentColor = if (item.pinned && !isSelected && !expanded && !detectedSensitive)
        IdeWarning.copy(alpha = 0.72f)
    else
        Color.Transparent

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .scale(rowScale)
            .background(rowBg)
            .drawBehind {
                // 2.dp left accent bar for pinned rows
                val barWidthPx = 2.dp.toPx()
                drawRect(
                    color = pinnedAccentColor,
                    size = androidx.compose.ui.geometry.Size(barWidthPx, size.height),
                )
            }
            .combinedClickable(
                interactionSource = interactionSource,
                indication = null, // press scale handles visual feedback
                onClick = {
                    if (selectionMode) {
                        onCheckboxTap()
                    } else if (detectedSensitive) {
                        onSensitiveTap()
                    } else {
                        onCopy()
                    }
                },
                onLongClick = {
                    if (!selectionMode) onLongPress()
                },
            )
            .padding(horizontal = 12.dp, vertical = 0.dp),
    ) {
        val bmp = imageBitmap
        if (item.isImage && bmp != null) {
            // ── Image thumbnail row ──────────────────────────────────────────
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                // Checkbox
                Icon(
                    imageVector = if (isSelected) Icons.Filled.CheckBox
                                  else Icons.Filled.CheckBoxOutlineBlank,
                    contentDescription = null,
                    tint = if (isSelected) IdeAccent else IdeDim.copy(alpha = 0.4f),
                    modifier = Modifier
                        .size(16.dp)
                        .clickable { onCheckboxTap() },
                )
                Spacer(Modifier.width(6.dp))
                if (!selectionMode && item.pinned) {
                    Icon(
                        imageVector = Icons.Filled.BookmarkAdded,
                        contentDescription = stringResource(R.string.cd_pin_item),
                        tint = IdeWarning.copy(alpha = 0.9f),
                        modifier = Modifier.size(12.dp),
                    )
                    Spacer(Modifier.width(4.dp))
                }
                // §5 content-type chip (violet for images)
                ContentTypeChip(contentType = item.contentType, isSensitive = detectedSensitive)
                if (!selectionMode && item.tooLargeToSync) TooLargeBadge()
                Spacer(Modifier.width(8.dp))
                Image(
                    bitmap = bmp,
                    contentDescription = stringResource(R.string.cd_image_thumbnail),
                    contentScale = ContentScale.Fit,
                    modifier = Modifier
                        .widthIn(max = 340.dp)
                        .heightIn(max = imageMaxHeightDp.dp)
                        .clip(RoundedCornerShape(4.dp))
                        .background(IdeElevated),
                )
                Spacer(Modifier.weight(1f))
                // §5 relative timestamp with tabular-nums via fontFeatureSettings
                Text(
                    text = relativeTime(item.wallTimeMs),
                    style = TextStyle(
                        fontSize = 11.sp,
                        fontWeight = FontWeight.Normal,
                        fontFeatureSettings = "tnum",
                    ),
                    color = IdeFaint,
                    maxLines = 1,
                )
                if (!selectionMode) {
                    Spacer(Modifier.width(4.dp))
                    if (reorderMode && item.pinned) {
                        ScaleIconButton(onClick = onMoveUp, modifier = Modifier.size(28.dp)) {
                            Icon(
                                imageVector = Icons.Filled.KeyboardArrowUp,
                                contentDescription = stringResource(R.string.action_move_up),
                                tint = if (pinnedIndex > 0) IdeAccent else IdeDim.copy(alpha = 0.3f),
                                modifier = Modifier.size(18.dp),
                            )
                        }
                        ScaleIconButton(onClick = onMoveDown, modifier = Modifier.size(28.dp)) {
                            Icon(
                                imageVector = Icons.Filled.KeyboardArrowDown,
                                contentDescription = stringResource(R.string.action_move_down),
                                tint = if (pinnedIndex < pinnedCount - 1) IdeAccent
                                       else IdeDim.copy(alpha = 0.3f),
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    } else {
                        ScaleIconButton(
                            onClick = { onSetPinned(item.id, !item.pinned) },
                        ) {
                            Icon(
                                imageVector = if (item.pinned) Icons.Filled.BookmarkAdded
                                              else Icons.Filled.BookmarkBorder,
                                contentDescription = if (item.pinned)
                                    stringResource(R.string.action_unpin)
                                else
                                    stringResource(R.string.action_pin),
                                tint = if (item.pinned) IdeWarning else IdeDim,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                        ScaleIconButton(
                            onClick = { onDelete(item.id) },
                        ) {
                            Icon(
                                imageVector = Icons.Filled.Delete,
                                contentDescription = stringResource(R.string.cd_delete),
                                tint = IdeDanger,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                    }
                }
            }
        } else if (item.isFile) {
            // ── File row — icon + filename label + Save action ────────────────
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(40.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                // Checkbox
                Icon(
                    imageVector = if (isSelected) Icons.Filled.CheckBox
                                  else Icons.Filled.CheckBoxOutlineBlank,
                    contentDescription = null,
                    tint = if (isSelected) IdeAccent else IdeDim.copy(alpha = 0.4f),
                    modifier = Modifier
                        .size(16.dp)
                        .clickable { onCheckboxTap() },
                )
                Spacer(Modifier.width(6.dp))
                if (!selectionMode && item.pinned) {
                    Icon(
                        imageVector = Icons.Filled.BookmarkAdded,
                        contentDescription = stringResource(R.string.cd_pin_item),
                        tint = IdeWarning.copy(alpha = 0.9f),
                        modifier = Modifier.size(12.dp),
                    )
                    Spacer(Modifier.width(4.dp))
                }
                // §3 content-type chip (file = dim/elevated)
                ContentTypeChip(contentType = item.contentType, isSensitive = detectedSensitive)
                if (!selectionMode && item.tooLargeToSync) TooLargeBadge()
                Spacer(Modifier.width(6.dp))
                // File icon
                Icon(
                    imageVector = Icons.Filled.AttachFile,
                    contentDescription = stringResource(R.string.cd_file_item),
                    tint = IdeDim,
                    modifier = Modifier.size(14.dp),
                )
                Spacer(Modifier.width(4.dp))
                // Filename / label — snippet holds "[file: name]" or "[file]"
                Text(
                    text = item.snippet,
                    style = MaterialTheme.typography.bodyLarge,
                    color = IdeText,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                Spacer(Modifier.width(6.dp))
                Text(
                    text = relativeTime(item.wallTimeMs),
                    style = TextStyle(
                        fontSize = 11.sp,
                        fontWeight = FontWeight.Normal,
                        fontFeatureSettings = "tnum",
                    ),
                    color = IdeFaint,
                    maxLines = 1,
                )
                if (!selectionMode) {
                    Spacer(Modifier.width(2.dp))
                    // Save action — write bytes to Downloads
                    ScaleIconButton(onClick = onSaveFile) {
                        Icon(
                            imageVector = Icons.Filled.SaveAlt,
                            contentDescription = stringResource(R.string.action_save_file),
                            tint = IdeAccent,
                            modifier = Modifier.size(16.dp),
                        )
                    }
                    ScaleIconButton(onClick = { onSetPinned(item.id, !item.pinned) }) {
                        Icon(
                            imageVector = if (item.pinned) Icons.Filled.BookmarkAdded
                                          else Icons.Filled.BookmarkBorder,
                            contentDescription = if (item.pinned)
                                stringResource(R.string.action_unpin)
                            else
                                stringResource(R.string.action_pin),
                            tint = if (item.pinned) IdeWarning else IdeDim,
                            modifier = Modifier.size(16.dp),
                        )
                    }
                    ScaleIconButton(onClick = { onDelete(item.id) }) {
                        Icon(
                            imageVector = Icons.Filled.Delete,
                            contentDescription = stringResource(R.string.cd_delete),
                            tint = IdeDanger,
                            modifier = Modifier.size(16.dp),
                        )
                    }
                }
            }
        } else {
            // ── Text row — §5 comfortable 40dp min height ─────────────────────
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(40.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                // Checkbox
                Icon(
                    imageVector = if (isSelected) Icons.Filled.CheckBox
                                  else Icons.Filled.CheckBoxOutlineBlank,
                    contentDescription = null,
                    tint = if (isSelected) IdeAccent else IdeDim.copy(alpha = 0.4f),
                    modifier = Modifier
                        .size(16.dp)
                        .clickable { onCheckboxTap() },
                )
                Spacer(Modifier.width(6.dp))
                if (!selectionMode && item.pinned) {
                    Icon(
                        imageVector = Icons.Filled.BookmarkAdded,
                        contentDescription = stringResource(R.string.cd_pin_item),
                        tint = IdeWarning.copy(alpha = 0.9f),
                        modifier = Modifier.size(12.dp),
                    )
                    Spacer(Modifier.width(4.dp))
                }
                // §5 content-type chip (tinted by type)
                ContentTypeChip(contentType = item.contentType, isSensitive = detectedSensitive)
                if (!selectionMode && item.tooLargeToSync) TooLargeBadge()
                Spacer(Modifier.width(8.dp))
                // Preview text
                Text(
                    text = display,
                    style = MaterialTheme.typography.bodyLarge,
                    color = if (detectedSensitive) IdeDim else IdeText,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                Spacer(Modifier.width(6.dp))
                // §5 source-app icon + label chip (right of text, left of timestamp)
                val ctx = LocalContext.current
                sourceAppLabel(item.sourceApp)?.let { appLabel ->
                    val iconBitmap by produceState<androidx.compose.ui.graphics.ImageBitmap?>(
                        initialValue = null,
                        key1 = item.sourceApp,
                    ) {
                        value = item.sourceApp?.let { pkg ->
                            withContext(Dispatchers.Default) {
                                runCatching {
                                    AppIconHelper.getAppIconBase64(ctx, pkg)
                                        ?.let { b64 ->
                                            val bytes = Base64.decode(b64, Base64.DEFAULT)
                                            BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
                                                ?.asImageBitmap()
                                        }
                                }.getOrElse { t ->
                                    // Fix P2: log icon load failures so they are diagnosable.
                                    AppLogger.w("HistoryRow", "app icon load failed for item ${item.id} pkg=$pkg", t)
                                    null
                                }
                            }
                        }
                    }
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier
                            .background(
                                color = IdeElevated.copy(alpha = 0.5f),
                                shape = RoundedCornerShape(4.dp),
                            )
                            .padding(horizontal = 4.dp, vertical = 2.dp),
                    ) {
                        iconBitmap?.let { iconBmp ->
                            Image(
                                bitmap = iconBmp,
                                contentDescription = null,
                                contentScale = ContentScale.Fit,
                                modifier = Modifier
                                    .size(14.dp)
                                    .clip(RoundedCornerShape(3.dp)),
                            )
                            Spacer(Modifier.width(3.dp))
                        }
                        Text(
                            text = appLabel,
                            style = TextStyle(fontSize = 10.sp, fontWeight = FontWeight.Normal),
                            color = IdeFaint,
                            maxLines = 1,
                        )
                    }
                    Spacer(Modifier.width(4.dp))
                }
                // §5 timestamp — always visible, tabular-nums
                Text(
                    text = relativeTime(item.wallTimeMs),
                    style = TextStyle(
                        fontSize = 11.sp,
                        fontWeight = FontWeight.Normal,
                        fontFeatureSettings = "tnum",
                    ),
                    color = IdeFaint,
                    maxLines = 1,
                )
                // Origin-device badge — shown when originDeviceId is known
                val originId = item.originDeviceId
                if (!selectionMode && originId != null && ownDeviceId.isNotBlank()) {
                    Spacer(Modifier.width(4.dp))
                    OriginDeviceBadge(
                        deviceId = originId,
                        ownDeviceId = ownDeviceId,
                        peers = peers,
                    )
                }
                if (!selectionMode) {
                    Spacer(Modifier.width(2.dp))
                    if (reorderMode && item.pinned) {
                        // Reorder mode: show up/down arrows instead of pin/delete
                        ScaleIconButton(
                            onClick = onMoveUp,
                            modifier = Modifier.size(28.dp),
                        ) {
                            Icon(
                                imageVector = Icons.Filled.KeyboardArrowUp,
                                contentDescription = stringResource(R.string.action_move_up),
                                tint = if (pinnedIndex > 0) IdeAccent else IdeDim.copy(alpha = 0.3f),
                                modifier = Modifier.size(18.dp),
                            )
                        }
                        ScaleIconButton(
                            onClick = onMoveDown,
                            modifier = Modifier.size(28.dp),
                        ) {
                            Icon(
                                imageVector = Icons.Filled.KeyboardArrowDown,
                                contentDescription = stringResource(R.string.action_move_down),
                                tint = if (pinnedIndex < pinnedCount - 1) IdeAccent
                                       else IdeDim.copy(alpha = 0.3f),
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    } else {
                        // §5 icon-only action buttons with press-scale (§8)
                        ScaleIconButton(onClick = { onSetPinned(item.id, !item.pinned) }) {
                            Icon(
                                imageVector = if (item.pinned) Icons.Filled.BookmarkAdded
                                              else Icons.Filled.BookmarkBorder,
                                contentDescription = if (item.pinned)
                                    stringResource(R.string.action_unpin)
                                else
                                    stringResource(R.string.action_pin),
                                tint = if (item.pinned) IdeWarning else IdeDim,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                        ScaleIconButton(onClick = { onDelete(item.id) }) {
                            Icon(
                                imageVector = Icons.Filled.Delete,
                                contentDescription = stringResource(R.string.cd_delete),
                                tint = IdeDanger,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                    }
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Device filter strip — parity with macOS HistoryView deviceFilter.
// Shown only when more than one origin device is present in the list.
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun DeviceFilterRow(
    deviceIds: Set<String>,
    selected: String,
    ownDeviceId: String,
    peers: List<PairedPeer>,
    onSelect: (String) -> Unit,
) {
    LazyRow(
        modifier = Modifier
            .fillMaxWidth()
            .background(IdePanel)
            .padding(horizontal = 12.dp, vertical = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        // "All" chip — always first
        item {
            DeviceChip(
                label = "All",
                isSelected = selected == "all",
                onClick = { onSelect("all") },
            )
        }
        // One chip per distinct origin device, own device first
        val sorted = deviceIds.sortedWith(
            compareByDescending<String> { it == ownDeviceId }
                .thenBy { deviceDisplayName(it, ownDeviceId, peers) }
        )
        items(sorted) { id ->
            DeviceChip(
                label = deviceDisplayName(id, ownDeviceId, peers),
                isSelected = selected == id,
                isOwn = id == ownDeviceId,
                onClick = { onSelect(id) },
            )
        }
    }
}

@Composable
private fun DeviceChip(
    label: String,
    isSelected: Boolean,
    isOwn: Boolean = false,
    onClick: () -> Unit,
) {
    val bg = when {
        isSelected -> IdeAccent
        isOwn      -> IdeAccentDim
        else       -> IdeElevated
    }
    val fg = if (isSelected) IdeBg else if (isOwn) IdeAccent else IdeDim

    Box(
        modifier = Modifier
            .background(color = bg, shape = RoundedCornerShape(12.dp))
            .clickable(onClick = onClick)
            .padding(horizontal = 10.dp, vertical = 4.dp),
    ) {
        Text(
            text = label,
            style = TextStyle(fontSize = 11.sp, fontWeight = FontWeight.Medium),
            color = fg,
            maxLines = 1,
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Origin-device badge — parity with macOS HistoryView DeviceBadge chip.
//
// Shown per-row when the item's [ClipboardItem.originDeviceId] is non-null.
// Displays "This device" (accented) for items captured locally, or the peer's
// display name (dim) for items received from another device.
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun OriginDeviceBadge(
    deviceId: String,
    ownDeviceId: String,
    peers: List<PairedPeer>,
) {
    val isOwn = deviceId == ownDeviceId
    val label = deviceDisplayName(deviceId, ownDeviceId, peers)

    Box(
        modifier = Modifier
            .background(
                color = if (isOwn) IdeAccentDim else IdeElevated,
                shape = RoundedCornerShape(4.dp),
            )
            .padding(horizontal = 4.dp, vertical = 2.dp),
    ) {
        Text(
            text = label,
            style = TextStyle(fontSize = 9.sp, fontWeight = FontWeight.Normal),
            color = if (isOwn) IdeAccent else IdeFaint,
            maxLines = 1,
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §8 ScaleIconButton — 28dp touch-target icon button with press-scale 0.98
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun ScaleIconButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()
    val scale by animateFloatAsState(
        targetValue = if (isPressed) 0.98f else 1.0f,
        animationSpec = tween(durationMillis = Motion.Instant, easing = EaseOutExpo),
        label = "btnScale",
    )
    IconButton(
        onClick = onClick,
        interactionSource = interactionSource,
        modifier = modifier
            .size(28.dp)
            .scale(scale),
    ) {
        content()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Type icon (legacy — used only when chip is not available in older paths)
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun TypeIcon(
    contentType: String,
    isSensitive: Boolean,
    modifier: Modifier = Modifier,
) {
    val (icon, tint) = when {
        isSensitive                          -> Icons.Filled.Lock to IdeDanger
        contentType.startsWith("image/") ||
            contentType == "image"           -> Icons.Filled.Image to IdeViolet
        contentType == "text" ||
            contentType.startsWith("text/")  -> Icons.Filled.ContentCopy to IdeAccent
        contentType == "url"                 -> Icons.Filled.ContentCopy to IdeInfo
        else                                 -> Icons.Filled.ContentCopy to IdeDim
    }
    Icon(
        imageVector = icon,
        contentDescription = null,
        tint = tint,
        modifier = modifier,
    )
}
