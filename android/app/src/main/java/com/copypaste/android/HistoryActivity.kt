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
import androidx.compose.animation.fadeIn
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
import androidx.compose.foundation.lazy.itemsIndexed
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
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
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

private enum class ConfirmAction { CLEAR_ALL, CLEAR_UNPINNED, DELETE_SELECTED }

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
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val loadErrorTemplate = stringResource(R.string.error_load_history)
    val dismissLabel = stringResource(R.string.snackbar_dismiss)
    val sensitiveTapMsg = stringResource(R.string.sensitive_tap_hint)

    // ── Search / filter state ────────────────────────────────────────────────
    var searchQuery by rememberSaveable { mutableStateOf("") }

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
    // Filter: case-insensitive substring match on snippet
    val filteredItems = remember(sortedItems, searchQuery) {
        val q = searchQuery.trim()
        if (q.isEmpty()) sortedItems
        else sortedItems.filter { it.snippet.contains(q, ignoreCase = true) }
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
                        // §9 search filter — desktop "Filter…" bar parity
                        OutlinedTextField(
                            value = searchQuery,
                            onValueChange = { searchQuery = it },
                            placeholder = {
                                Text(
                                    text = stringResource(R.string.history_filter_placeholder),
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = IdeFaint,
                                )
                            },
                            singleLine = true,
                            colors = ideTextFieldColors(),
                            modifier = Modifier
                                .width(160.dp)
                                .height(44.dp),
                            textStyle = MaterialTheme.typography.bodyMedium.copy(color = IdeText),
                        )
                        Spacer(Modifier.width(4.dp))
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
            // §9: history completely empty
            sortedItems.isEmpty() -> EmptyHistoryState(innerPadding)
            // §9: search returned no results
            filteredItems.isEmpty() -> EmptySearchState(innerPadding, searchQuery.trim())
            else -> HistoryList(
                items = filteredItems,
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
        // §8 mount fade/rise stagger — AnimatedVisibility per item, capped at 10 items
        // to avoid stagger on large existing lists (only on initial appearance).
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
                Column {
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
                            scope.launch {
                                val key = settings.encryptionKey
                                val fullText = repository.loadFullPlaintext(item.id, key)
                                    ?: item.snippet
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

    // §5 row background: selection > expanded > sensitive tint > transparent
    val rowBg = when {
        isSelected        -> IdeSelection
        expanded          -> IdeElevated
        detectedSensitive -> IdeDanger.copy(alpha = 0.07f)
        item.pinned       -> IdeWarning.copy(alpha = 0.06f)
        else              -> Color.Transparent
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .scale(rowScale)
            .background(rowBg)
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
                                }.getOrNull()
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
                if (!selectionMode) {
                    Spacer(Modifier.width(2.dp))
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
