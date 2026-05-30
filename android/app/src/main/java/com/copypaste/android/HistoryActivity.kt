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
import androidx.compose.foundation.Image
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
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
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
 * History screen — Compose list of clipboard items with macOS parity.
 *
 * Row behaviour:
 *   - Tapping a row copies the item (single-tap = copy, no explicit Copy button)
 *   - Per-row checkbox (always visible) — tapping it enters multi-select mode
 *   - Long-press also enters multi-select mode and selects the tapped row
 *   - In selection mode: bulk action bar replaces the top bar (delete/pin)
 *   - Action buttons on expand: icon-only pin/unpin + delete (no text labels)
 *   - Timestamp always visible in the right gutter
 *   - Pinned items shown with a bookmark indicator
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

    // ── Selection state (survives rotation) ─────────────────────────────────
    var selectionMode by rememberSaveable { mutableStateOf(false) }
    var selectedIds by rememberSaveable(
        stateSaver = listSaver(
            save    = { it.toList() },
            restore = { it.toSet() },
        )
    ) { mutableStateOf(setOf<String>()) }

    var pendingConfirm by remember { mutableStateOf<ConfirmAction?>(null) }
    var overflowExpanded by remember { mutableStateOf(false) }

    // Sort: pinned first, then by recency
    val sortedItems = remember(items) {
        items.sortedWith(
            compareByDescending<ClipboardItem> { it.pinned }
                .thenByDescending { it.wallTimeMs }
        )
    }

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
                                            Icon(Icons.Filled.DeleteSweep, null, tint = IdeDanger)
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
                    selectionMode = true
                    selectedIds = setOf(id)
                },
                onCheckboxTap = { id ->
                    // Tapping the checkbox always enters/toggles selection mode
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
    onCheckboxTap: (String) -> Unit,
    onSensitiveTap: () -> Unit = {},
) {
    val ctx = LocalContext.current
    val settings = remember { Settings(ctx) }
    val maskSensitive = remember { settings.maskSensitiveContent }
    val imageMaxHeightDp = remember { settings.imageMaxHeight }
    val previewDelayMs = remember { settings.previewDelay }
    // Repository and key used to load FULL plaintext on copy — not the 140-char snippet.
    val repository = remember { ClipboardRepository(ctx) }
    val scope = rememberCoroutineScope()

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
                onCopy = {
                    // Load the FULL decrypted plaintext before writing to the
                    // system clipboard — item.snippet is capped at 140 chars and
                    // would truncate the user's actual content.
                    scope.launch {
                        val key = settings.encryptionKey
                        val fullText = repository.loadFullPlaintext(item.id, key)
                            ?: item.snippet  // fallback to snippet if decrypt fails
                        val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                        cm.setPrimaryClip(ClipData.newPlainText("CopyPaste", fullText))
                    }
                },
                onLongPress = { onLongPress(item.id) },
                onCheckboxTap = { onCheckboxTap(item.id) },
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
// Row
//
// Layout (left→right): [checkbox] [pin-badge?] [type-icon] [preview] [timestamp] [icon-actions]
//
// Single-tap: copies item (non-sensitive). Sensitive items: shows hint snackbar.
// In selection mode: single-tap toggles checkbox.
// Checkbox tap: always enters/toggles selection mode.
// Long-press: enters selection mode (normal) or no-op (already selecting).
// Action icon buttons (pin/unpin + delete): ICON-ONLY, compact, right-aligned.
// Auto-collapse action row after previewDelayMs of inactivity.
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
    onCopy: () -> Unit = {},
    onLongPress: () -> Unit,
    onCheckboxTap: () -> Unit,
    onSensitiveTap: () -> Unit = {},
) {
    val detectedSensitive = item.isSensitive

    var expanded by remember(item.id) { mutableStateOf(false) }
    LaunchedEffect(expanded) {
        if (expanded) {
            delay(previewDelayMs)
            expanded = false
        }
    }
    LaunchedEffect(selectionMode) {
        if (selectionMode) expanded = false
    }

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
        isSelected        -> IdeAccent.copy(alpha = 0.15f)
        expanded          -> IdeSelection
        detectedSensitive -> IdeDanger.copy(alpha = 0.07f)
        else              -> Color.Transparent
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(rowBg)
            .combinedClickable(
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
                // Checkbox — always visible, tapping it enters/toggles selection
                Icon(
                    imageVector = if (isSelected) Icons.Filled.CheckBox
                                  else Icons.Filled.CheckBoxOutlineBlank,
                    contentDescription = null,
                    tint = if (isSelected) IdeAccent else IdeDim.copy(alpha = 0.4f),
                    modifier = Modifier
                        .size(16.dp)
                        .clickable { onCheckboxTap() },
                )
                Spacer(Modifier.width(4.dp))
                if (!selectionMode && item.pinned) {
                    Icon(
                        imageVector = Icons.Filled.BookmarkAdded,
                        contentDescription = stringResource(R.string.cd_pin_item),
                        tint = IdeAccent.copy(alpha = 0.7f),
                        modifier = Modifier.size(12.dp),
                    )
                    Spacer(Modifier.width(4.dp))
                }
                Icon(
                    imageVector = Icons.Filled.Image,
                    contentDescription = stringResource(R.string.cd_image_item),
                    tint = IdeAccent,
                    modifier = Modifier.size(14.dp),
                )
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
                Text(
                    text = formatTime(item.wallTimeMs),
                    style = MaterialTheme.typography.bodyMedium,
                    color = IdeFaint,
                    maxLines = 1,
                )
                if (!selectionMode) {
                    Spacer(Modifier.width(6.dp))
                    // Icon-only action buttons: pin/unpin + delete
                    IconButton(
                        onClick = { onSetPinned(item.id, !item.pinned) },
                        modifier = Modifier.size(28.dp),
                    ) {
                        Icon(
                            imageVector = if (item.pinned) Icons.Filled.BookmarkAdded
                                          else Icons.Filled.BookmarkBorder,
                            contentDescription = if (item.pinned)
                                stringResource(R.string.action_unpin)
                            else
                                stringResource(R.string.action_pin),
                            tint = if (item.pinned) IdeAccent else IdeDim,
                            modifier = Modifier.size(16.dp),
                        )
                    }
                    IconButton(
                        onClick = { onDelete(item.id) },
                        modifier = Modifier.size(28.dp),
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
        } else {
            // ── Text row ─────────────────────────────────────────────────────
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(40.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                // Checkbox — always visible
                Icon(
                    imageVector = if (isSelected) Icons.Filled.CheckBox
                                  else Icons.Filled.CheckBoxOutlineBlank,
                    contentDescription = null,
                    tint = if (isSelected) IdeAccent else IdeDim.copy(alpha = 0.4f),
                    modifier = Modifier
                        .size(16.dp)
                        .clickable { onCheckboxTap() },
                )
                Spacer(Modifier.width(4.dp))
                if (!selectionMode && item.pinned) {
                    Icon(
                        imageVector = Icons.Filled.BookmarkAdded,
                        contentDescription = stringResource(R.string.cd_pin_item),
                        tint = IdeAccent.copy(alpha = 0.7f),
                        modifier = Modifier.size(12.dp),
                    )
                    Spacer(Modifier.width(4.dp))
                }
                TypeIcon(
                    contentType = item.contentType,
                    isSensitive = detectedSensitive,
                    modifier = Modifier.size(14.dp),
                )
                Spacer(Modifier.width(8.dp))
                Text(
                    text = display,
                    style = MaterialTheme.typography.bodyLarge,
                    color = if (detectedSensitive) IdeDim else IdeText,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                Spacer(Modifier.width(8.dp))
                // Source-app icon + label — only when sourceApp is known.
                // Icon is decoded off the main thread via produceState; AppIconHelper
                // caches results so repeat rows are cheap.
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
                                }.getOrNull()
                            }
                        }
                    }
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.padding(end = 4.dp),
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
                            style = MaterialTheme.typography.labelSmall,
                            color = IdeFaint.copy(alpha = 0.65f),
                            maxLines = 1,
                        )
                    }
                }
                // Timestamp always visible
                Text(
                    text = formatTime(item.wallTimeMs),
                    style = MaterialTheme.typography.bodyMedium,
                    color = IdeFaint,
                    maxLines = 1,
                )
                if (!selectionMode) {
                    Spacer(Modifier.width(4.dp))
                    // Icon-only action buttons: pin/unpin + delete (compact, no text)
                    IconButton(
                        onClick = { onSetPinned(item.id, !item.pinned) },
                        modifier = Modifier.size(28.dp),
                    ) {
                        Icon(
                            imageVector = if (item.pinned) Icons.Filled.BookmarkAdded
                                          else Icons.Filled.BookmarkBorder,
                            contentDescription = if (item.pinned)
                                stringResource(R.string.action_unpin)
                            else
                                stringResource(R.string.action_pin),
                            tint = if (item.pinned) IdeAccent else IdeDim,
                            modifier = Modifier.size(16.dp),
                        )
                    }
                    IconButton(
                        onClick = { onDelete(item.id) },
                        modifier = Modifier.size(28.dp),
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
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Type icon
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

private fun formatTime(ms: Long): String =
    DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT).format(Date(ms))
