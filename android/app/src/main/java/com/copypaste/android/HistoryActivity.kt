@file:OptIn(ExperimentalFoundationApi::class)

package com.copypaste.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.ExperimentalFoundationApi
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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
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
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
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
import com.copypaste.android.ui.theme.IdeFaint
import com.copypaste.android.ui.theme.IdePanel
import com.copypaste.android.ui.theme.IdeSelection
import com.copypaste.android.ui.theme.IdeText
import kotlinx.coroutines.delay
import java.text.DateFormat
import java.util.Date

/**
 * History screen — Compose list of last [HISTORY_LIMIT] clipboard items.
 *
 * Redesigned to match the macOS desktop UI:
 *   - Compact 44 dp header bar (IDE-style, 14 sp medium title)
 *   - Dark Darcula background (#2b2d30 surface, #1e1f22 list bg)
 *   - Rows: left-aligned type icon + text preview (13 sp) + right faint timestamp
 *   - Thin dividers between rows (no card elevation)
 *   - Long-press reveals delete/pin actions (avoids permanent icons on every row)
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
    val loadErrorTemplate = stringResource(R.string.error_load_history)
    val dismissLabel = stringResource(R.string.snackbar_dismiss)

    // ── Selection state ──────────────────────────────────────────────────────
    // rememberSaveable so it survives recomposition but resets on back-stack pop.
    var selectionMode by rememberSaveable { mutableStateOf(false) }
    var selectedIds by remember { mutableStateOf(setOf<String>()) }

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
                        containerColor = IdePanel,
                        titleContentColor = IdeText,
                        actionIconContentColor = IdeDim,
                        navigationIconContentColor = IdeDim,
                    ),
                    // Apply the status-bar / display-cutout inset as TOP PADDING so the
                    // bar's content sits *below* the notch, never under it.
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
) {
    val ctx = LocalContext.current
    val maskSensitive = remember { Settings(ctx).maskSensitiveContent }

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
                selectionMode = selectionMode,
                isSelected = selectedIds.contains(item.id),
                onDelete = onDelete,
                onSetPinned = onSetPinned,
                onLongPress = { onLongPress(item.id) },
                onToggleSelect = { onToggleSelect(item.id) },
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
// Normal mode: long-press toggles action row (delete / copy / pin).
// Selection mode: tap toggles checkbox; long-press is a no-op (already in mode).
// ─────────────────────────────────────────────────────────────────────────────

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun HistoryRow(
    item: ClipboardItem,
    maskSensitive: Boolean,
    selectionMode: Boolean,
    isSelected: Boolean,
    onDelete: (String) -> Unit,
    onSetPinned: (String, Boolean) -> Unit,
    onLongPress: () -> Unit,
    onToggleSelect: () -> Unit,
) {
    // Sensitivity flag is set at load time against the full plaintext — trust it.
    val detectedSensitive = item.isSensitive

    var expanded by remember(item.id) { mutableStateOf(false) }
    // Auto-collapse the action row after 4 s of inactivity.
    LaunchedEffect(expanded) {
        if (expanded) {
            delay(4_000L)
            expanded = false
        }
    }
    // Collapse action row when entering selection mode
    LaunchedEffect(selectionMode) {
        if (selectionMode) expanded = false
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
                onClick = {
                    if (selectionMode) {
                        onToggleSelect()
                    }
                    // In normal mode single-tap is no-op on the row itself
                },
                onLongClick = {
                    if (selectionMode) {
                        // Already in selection mode — long-press is no-op
                    } else {
                        onLongPress()
                    }
                },
            )
            .padding(horizontal = 12.dp, vertical = 0.dp),
    ) {
        // ── Main row (always visible) ────────────────────────────────────
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
                // Small pin indicator for pinned items
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

            // Timestamp — right-aligned, faint (matches macOS group-hover:hidden)
            if (!expanded && !selectionMode) {
                Text(
                    text = formatTime(item.wallTimeMs),
                    style = MaterialTheme.typography.bodyMedium,
                    color = IdeFaint,
                    maxLines = 1,
                )
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
        isSensitive              -> Icons.Filled.Lock to IdeDanger
        contentType == "text"    -> Icons.Filled.ContentCopy to IdeAccent
        else                     -> Icons.Filled.ContentCopy to IdeDim
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
