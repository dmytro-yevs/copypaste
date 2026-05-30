@file:OptIn(ExperimentalFoundationApi::class)

package com.copypaste.android

import android.graphics.BitmapFactory
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
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
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.BookmarkAdded
import androidx.compose.material.icons.filled.BookmarkBorder
import androidx.compose.material.icons.filled.CheckBox
import androidx.compose.material.icons.filled.CheckBoxOutlineBlank
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.DeleteSweep
import androidx.compose.material.icons.filled.Image
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.AlertDialog
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.livedata.observeAsState
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.IdeAccent
import com.copypaste.android.ui.theme.IdeBg
import com.copypaste.android.ui.theme.IdeBorder
import com.copypaste.android.ui.theme.IdeDanger
import com.copypaste.android.ui.theme.IdeDim
import com.copypaste.android.ui.theme.IdeElevated
import com.copypaste.android.ui.theme.IdeFaint
import com.copypaste.android.ui.theme.IdePanel
import com.copypaste.android.ui.theme.IdeSelection
import com.copypaste.android.ui.theme.IdeText
import kotlinx.coroutines.delay
import java.text.DateFormat
import java.util.Date

/**
 * History screen — Compose list of last N clipboard items, where N is read
 * live from [Settings.historySize] (Maccy-parity display cap).
 *
 * Redesigned to match the macOS desktop UI:
 *   - Compact 44 dp header bar (IDE-style, 14 sp medium title)
 *   - Dark Darcula background (#2b2d30 surface, #1e1f22 list bg)
 *   - Image items: thumbnail scaled into 340 dp × imageMaxHeight dp bounding
 *     box (ContentScale.Fit, never upscaled) instead of a text preview
 *   - Text items: left-aligned type icon + text preview (13 sp) + right faint ts
 *   - Thin dividers between rows (no card elevation)
 *   - Long-press reveals delete/pin actions; auto-collapses after [Settings.previewDelay]
 *   - Pinned items shown first, with a pin indicator icon
 *   - Bulk multi-select mode: long-press enters selection, contextual top bar
 *   - Clear All / Clear Unpinned in overflow menu with confirmation dialog
 */
class HistoryActivity : ComponentActivity() {

    private val viewModel: ClipboardViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Draw edge-to-edge so the dark background fills the status-bar / cutout
        // strip; the screen's TopAppBar applies the matching inset as padding.
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
// Confirmation dialog enum — tracks which destructive action is pending
// ─────────────────────────────────────────────────────────────────────────────

private enum class ConfirmAction { CLEAR_ALL, CLEAR_UNPINNED, DELETE_SELECTED }

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
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val loadErrorTemplate = stringResource(R.string.error_load_history)
    val dismissLabel = stringResource(R.string.snackbar_dismiss)
    val sensitiveTapMsg = stringResource(R.string.sensitive_tap_hint)

    // ── Selection state ──────────────────────────────────────────────────────
    // Both survive config changes (rotation) via rememberSaveable.
    // selectedIds uses a listSaver to round-trip Set<String> through a List<String>
    // (List is a supported saveable type; Set is not directly saveable).
    var selectionMode by rememberSaveable { mutableStateOf(false) }
    var selectedIds by rememberSaveable(
        stateSaver = listSaver(
            save    = { it.toList() },
            restore = { it.toSet() },
        )
    ) { mutableStateOf(setOf<String>()) }

    // ── Confirmation dialog state ────────────────────────────────────────────
    var pendingConfirm by remember { mutableStateOf<ConfirmAction?>(null) }

    // ── Overflow menu state ──────────────────────────────────────────────────
    var overflowExpanded by remember { mutableStateOf(false) }

    // Sort: pinned items first, then by recency (descending wallTimeMs)
    val sortedItems = remember(items) {
        items.sortedWith(
            compareByDescending<ClipboardItem> { it.pinned }
                .thenByDescending { it.wallTimeMs }
        )
    }

    // Exit selection mode when navigating back
    BackHandler(enabled = selectionMode) {
        selectionMode = false
        selectedIds = emptySet()
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
                ConfirmAction.CLEAR_ALL -> items.size
                ConfirmAction.CLEAR_UNPINNED -> items.count { !it.pinned }
                ConfirmAction.DELETE_SELECTED -> selectedIds.size
            },
            onConfirm = {
                pendingConfirm = null
                when (action) {
                    ConfirmAction.CLEAR_ALL -> viewModel.clearAll()
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
                // ── Contextual selection top bar ─────────────────────────────
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
                        // Pin all selected that are not yet pinned
                        selectedIds.forEach { id ->
                            val item = sortedItems.find { it.id == id }
                            if (item != null && !item.pinned) {
                                viewModel.setPinned(id, true)
                            }
                        }
                        selectionMode = false
                        selectedIds = emptySet()
                    },
                    onUnpinSelected = {
                        // Unpin all selected that are pinned
                        selectedIds.forEach { id ->
                            val item = sortedItems.find { it.id == id }
                            if (item != null && item.pinned) {
                                viewModel.setPinned(id, false)
                            }
                        }
                        selectionMode = false
                        selectedIds = emptySet()
                    },
                )
            } else {
                // ── Normal top bar ───────────────────────────────────────────
                TopAppBar(
                    title = {
                        Text(
                            text = stringResource(R.string.title_history),
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
                        IconButton(onClick = { viewModel.loadItems() }) {
                            Icon(
                                Icons.Filled.Refresh,
                                contentDescription = stringResource(R.string.cd_refresh),
                                tint = IdeDim,
                                modifier = Modifier.size(18.dp),
                            )
                        }
                        // Overflow menu: Clear All / Clear Unpinned
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
                                    DropdownMenuItem(
                                        text = {
                                            Text(
                                                stringResource(R.string.action_clear_all),
                                                color = IdeDanger,
                                            )
                                        },
                                        leadingIcon = {
                                            Icon(
                                                Icons.Filled.DeleteSweep,
                                                contentDescription = null,
                                                tint = IdeDanger,
                                            )
                                        },
                                        onClick = {
                                            overflowExpanded = false
                                            pendingConfirm = ConfirmAction.CLEAR_ALL
                                        },
                                    )
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
                                                Icon(
                                                    Icons.Filled.Delete,
                                                    contentDescription = null,
                                                    tint = IdeDim,
                                                )
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
                    // Apply the status-bar / display-cutout inset as TOP PADDING so the
                    // bar's content sits *below* the notch, never under it. We must NOT
                    // pin a fixed total height here (that was the bug: a hard 44 dp
                    // clipped the header on notched phones because the inset ate into
                    // it). The bar now measures as (status-bar inset + compact content)
                    // and the default M3 TopAppBar height keeps it visually compact.
                    windowInsets = TopAppBarDefaults.windowInsets,
                )
            }
        },
        snackbarHost = { SnackbarHost(hostState = snackbarHostState) },
    ) { innerPadding ->
        when {
            loading -> LoadingBox(innerPadding)
            sortedItems.isEmpty() -> EmptyState(innerPadding)
            else -> HistoryList(
                items = sortedItems,
                padding = innerPadding,
                selectionMode = selectionMode,
                selectedIds = selectedIds,
                onDelete = { id -> viewModel.deleteItem(id) },
                onSetPinned = { id, pinned -> viewModel.setPinned(id, pinned) },
                onLongPress = { id ->
                    // Long-press on a row: enter selection mode and select this item
                    selectionMode = true
                    selectedIds = setOf(id)
                },
                onToggleSelect = { id ->
                    selectedIds = if (selectedIds.contains(id)) {
                        selectedIds - id
                    } else {
                        selectedIds + id
                    }
                    // Auto-exit selection mode when last item deselected
                    if (selectedIds.isEmpty()) {
                        selectionMode = false
                    }
                },
                onSensitiveTap = {
                    scope.launch {
                        snackbarHostState.showSnackbar(sensitiveTapMsg)
                    }
                },
            )
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Contextual selection top bar
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
            // Toggle select-all / deselect-all
            val allSelected = selectedCount == totalCount && totalCount > 0
            IconButton(onClick = onSelectAll) {
                Icon(
                    if (allSelected) Icons.Filled.CheckBox else Icons.Filled.CheckBoxOutlineBlank,
                    contentDescription = stringResource(R.string.cd_select_all),
                    tint = if (allSelected) IdeAccent else IdeDim,
                    modifier = Modifier.size(18.dp),
                )
            }
            // Pin selected
            if (selectedCount > 0) {
                IconButton(onClick = onPinSelected) {
                    Icon(
                        Icons.Filled.BookmarkAdded,
                        contentDescription = stringResource(R.string.action_pin_selected),
                        tint = IdeAccent,
                        modifier = Modifier.size(18.dp),
                    )
                }
                // Unpin selected
                IconButton(onClick = onUnpinSelected) {
                    Icon(
                        Icons.Filled.BookmarkBorder,
                        contentDescription = stringResource(R.string.action_unpin_selected),
                        tint = IdeDim,
                        modifier = Modifier.size(18.dp),
                    )
                }
                // Delete selected
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
        colors = TopAppBarDefaults.topAppBarColors(
            containerColor = IdeSelection,
            titleContentColor = IdeText,
            actionIconContentColor = IdeDim,
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
        ConfirmAction.CLEAR_ALL -> stringResource(R.string.dialog_clear_all_title)
        ConfirmAction.CLEAR_UNPINNED -> stringResource(R.string.dialog_clear_unpinned_title)
        ConfirmAction.DELETE_SELECTED -> stringResource(R.string.dialog_delete_selected_title)
    }
    val message = when (action) {
        ConfirmAction.CLEAR_ALL ->
            stringResource(R.string.dialog_clear_all_message, itemCount)
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
                Text(
                    stringResource(R.string.dialog_confirm),
                    color = IdeDanger,
                )
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(
                    stringResource(R.string.dialog_cancel),
                    color = IdeDim,
                )
            }
        },
        containerColor = IdePanel,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Loading / empty states
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

@Composable
private fun EmptyState(padding: PaddingValues) {
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(IdeBg)
            .padding(padding)
            .padding(24.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = stringResource(R.string.empty_history),
            style = MaterialTheme.typography.bodyLarge,
            color = IdeFaint,
        )
    }
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
    onDelete: (String) -> Unit,
    onSetPinned: (String, Boolean) -> Unit,
    onLongPress: (String) -> Unit,
    onToggleSelect: (String) -> Unit,
    onSensitiveTap: () -> Unit = {},
) {
    val ctx = LocalContext.current
    val settings = remember { Settings(ctx) }
    val maskSensitive = remember { settings.maskSensitiveContent }

    // Read display settings live so the list responds to Settings changes
    // without requiring a full history reload. Both values are read once per
    // composition; they update on the next compose frame if settings change
    // (the settings screen writes immediately via SharedPreferences).
    val imageMaxHeightDp = remember { settings.imageMaxHeight }
    val previewDelayMs = remember { settings.previewDelay }

    LazyColumn(
        modifier = Modifier
            .fillMaxSize()
            .background(IdeBg)
            .padding(padding),
        contentPadding = PaddingValues(0.dp),
        verticalArrangement = Arrangement.spacedBy(0.dp),
    ) {
        items(items, key = { it.id }) { item ->
            HistoryRow(
                item = item,
                maskSensitive = maskSensitive,
                imageMaxHeightDp = imageMaxHeightDp,
                previewDelayMs = previewDelayMs,
                selectionMode = selectionMode,
                isSelected = selectedIds.contains(item.id),
                onDelete = onDelete,
                onSetPinned = onSetPinned,
                onCopy = { snippet ->
                    val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                    cm.setPrimaryClip(ClipData.newPlainText("CopyPaste", snippet))
                },
                onLongPress = { onLongPress(item.id) },
                onToggleSelect = { onToggleSelect(item.id) },
                onSensitiveTap = onSensitiveTap,
            )
            HorizontalDivider(
                color = IdeBorder.copy(alpha = 0.5f),
                thickness = 0.5.dp,
            )
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Row — compact IDE-style, matching the macOS HistoryRow layout:
//   [checkbox?] [pin?] [type-icon]  [preview ─────────────]  [timestamp]  [actions]
//
// Image items: thumbnail shown in a bounding box (340 dp × imageMaxHeightDp dp).
// Text items (and image-no-bytes fallback): standard compact 36 dp text row.
//
// Normal mode: single-tap copies (non-sensitive); long-press enters selection.
// Selection mode: tap toggles checkbox; long-press is a no-op.
// Action row (long-press in normal mode): pin/unpin, copy, delete chips.
// Auto-collapses after [previewDelayMs] of inactivity (Maccy previewDelay parity).
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
    onDelete: (String) -> Unit,
    onSetPinned: (String, Boolean) -> Unit,
    onCopy: (String) -> Unit = {},
    onLongPress: () -> Unit,
    onToggleSelect: () -> Unit,
    onSensitiveTap: () -> Unit = {},
) {
    // Sensitivity flag is set at load time against the full plaintext — trust it.
    val detectedSensitive = item.isSensitive

    var expanded by remember(item.id) { mutableStateOf(false) }
    // Auto-collapse after previewDelayMs of inactivity (Maccy previewDelay parity).
    LaunchedEffect(expanded) {
        if (expanded) {
            delay(previewDelayMs)
            expanded = false
        }
    }
    // Collapse action row when entering selection mode
    LaunchedEffect(selectionMode) {
        if (selectionMode) expanded = false
    }

    // Decode image bytes off the main thread to avoid jank while scrolling.
    // produceState re-runs whenever item.id or item.imagePng changes; null while
    // decoding (shows nothing / text fallback) and after any decode failure.
    val imageBitmap by produceState<androidx.compose.ui.graphics.ImageBitmap?>(
        initialValue = null,
        key1 = item.id,
        key2 = item.imagePng,
    ) {
        value = item.imagePng?.let { bytes ->
            withContext(Dispatchers.Default) {
                runCatching {
                    BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.asImageBitmap()
                }.getOrNull()
            }
        }
    }

    val maskString = stringResource(R.string.sensitive_preview_mask)
    val display = when {
        detectedSensitive && maskSensitive -> maskString
        item.snippet.isBlank() -> stringResource(R.string.empty_history)
        else -> item.snippet
    }
    val rowBg = when {
        isSelected         -> IdeAccent.copy(alpha = 0.15f)
        expanded           -> IdeSelection
        detectedSensitive  -> IdeDanger.copy(alpha = 0.07f)
        else               -> Color.Transparent
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(rowBg)
            .combinedClickable(
                // Single-tap: toggle selection in selection mode; copy non-sensitive in normal mode.
                // Sensitive items in normal mode: fire onSensitiveTap so the caller can surface
                // brief feedback ("Sensitive — use Copy action") rather than silently no-oping.
                onClick = {
                    if (selectionMode) {
                        onToggleSelect()
                    } else if (detectedSensitive) {
                        onSensitiveTap()
                    } else {
                        onCopy(item.snippet)
                    }
                },
                // Long-press: enter selection mode (normal) or no-op (already selecting).
                onLongClick = {
                    if (!selectionMode) {
                        onLongPress()
                    }
                },
            )
            .padding(horizontal = 12.dp, vertical = 0.dp),
    ) {
        val bmp = imageBitmap
        if (item.isImage && bmp != null) {
            // ── Image thumbnail row ──────────────────────────────────────────
            // Bounding box: max 340 dp wide × imageMaxHeightDp dp tall.
            // ContentScale.Fit = uniform scale-down to fit inside the box.
            // NEVER upscale: widthIn/heightIn use max constraints only, so an
            // image smaller than the box is shown at its natural size.
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.Start,
            ) {
                // Checkbox (selection mode) or pin indicator (normal mode)
                if (selectionMode) {
                    Icon(
                        imageVector = if (isSelected) Icons.Filled.CheckBox
                                      else Icons.Filled.CheckBoxOutlineBlank,
                        contentDescription = null,
                        tint = if (isSelected) IdeAccent else IdeDim,
                        modifier = Modifier.size(16.dp),
                    )
                    Spacer(Modifier.width(6.dp))
                } else if (item.pinned) {
                    Icon(
                        imageVector = Icons.Filled.BookmarkAdded,
                        contentDescription = stringResource(R.string.cd_pin_item),
                        tint = IdeAccent.copy(alpha = 0.7f),
                        modifier = Modifier.size(12.dp),
                    )
                    Spacer(Modifier.width(4.dp))
                }
                // Small image-type icon to the left, matching the text-row icon gutter
                Icon(
                    imageVector = Icons.Filled.Image,
                    contentDescription = stringResource(R.string.cd_image_item),
                    tint = IdeAccent,
                    modifier = Modifier.size(14.dp),
                )
                Spacer(Modifier.width(8.dp))
                // Thumbnail — scales uniformly into the bounding box, never upscales
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
                if (!expanded && !selectionMode) {
                    Text(
                        text = formatTime(item.wallTimeMs),
                        style = MaterialTheme.typography.bodyMedium,
                        color = IdeFaint,
                        maxLines = 1,
                    )
                }
            }
        } else {
            // ── Text (or image-no-bytes fallback) row ────────────────────────
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(36.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.Start,
            ) {
                // Checkbox (selection mode) or pin indicator (normal mode)
                if (selectionMode) {
                    Icon(
                        imageVector = if (isSelected) Icons.Filled.CheckBox
                                      else Icons.Filled.CheckBoxOutlineBlank,
                        contentDescription = null,
                        tint = if (isSelected) IdeAccent else IdeDim,
                        modifier = Modifier.size(16.dp),
                    )
                    Spacer(Modifier.width(6.dp))
                } else if (item.pinned) {
                    Icon(
                        imageVector = Icons.Filled.BookmarkAdded,
                        contentDescription = stringResource(R.string.cd_pin_item),
                        tint = IdeAccent.copy(alpha = 0.7f),
                        modifier = Modifier.size(12.dp),
                    )
                    Spacer(Modifier.width(4.dp))
                }

                // Type icon glyph (16 dp, left-pinned like macOS)
                TypeIcon(
                    contentType = item.contentType,
                    isSensitive = detectedSensitive,
                    modifier = Modifier.size(14.dp),
                )

                Spacer(Modifier.width(8.dp))

                // Preview text — single line with ellipsis, flex-1
                Text(
                    text = display,
                    style = MaterialTheme.typography.bodyLarge,
                    color = if (detectedSensitive) IdeDim else IdeText,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )

                Spacer(Modifier.width(8.dp))

                // Timestamp — right-aligned, faint
                if (!expanded && !selectionMode) {
                    Text(
                        text = formatTime(item.wallTimeMs),
                        style = MaterialTheme.typography.bodyMedium,
                        color = IdeFaint,
                        maxLines = 1,
                    )
                }
            }
        }

        // ── Action row (visible on long-press in normal mode) ────────────
        if (expanded && !selectionMode) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(bottom = 6.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.End),
            ) {
                // Content-type label (subdued, left side)
                Text(
                    text = item.contentType,
                    style = MaterialTheme.typography.bodyMedium,
                    color = IdeFaint,
                    modifier = Modifier.weight(1f),
                )
                // Pin / Unpin toggle
                ActionChip(
                    label = if (item.pinned) stringResource(R.string.action_unpin)
                            else stringResource(R.string.action_pin),
                    danger = false,
                    onClick = { onSetPinned(item.id, !item.pinned) },
                )
                ActionChip(
                    label = stringResource(R.string.cd_copy),
                    onClick = { onCopy(item.snippet); expanded = false },
                )
                ActionChip(
                    label = stringResource(R.string.cd_delete),
                    danger = true,
                    onClick = { onDelete(item.id) },
                )
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Type icon — small colored glyph matching macOS ContentIcon colors
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
            contentType == "image"           -> Icons.Filled.Image to IdeAccent
        contentType == "text" ||
            contentType.startsWith("text/")  -> Icons.Filled.ContentCopy to IdeAccent
        else                                 -> Icons.Filled.ContentCopy to IdeDim
    }
    Icon(
        imageVector = icon,
        contentDescription = null,
        tint = tint,
        modifier = modifier,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Action chip — compact pill button, matches macOS ActionBtn style
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun ActionChip(
    label: String,
    danger: Boolean = false,
    onClick: () -> Unit,
) {
    val textColor = if (danger) IdeDanger else IdeText
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(4.dp))
            .background(IdePanel)
            .clickable(onClick = onClick)
            .padding(horizontal = 10.dp, vertical = 4.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelLarge,
            color = textColor,
        )
    }
}

private fun formatTime(ms: Long): String =
    DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT).format(Date(ms))
