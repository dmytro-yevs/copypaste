@file:OptIn(ExperimentalFoundationApi::class)

package com.copypaste.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
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
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.livedata.observeAsState
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
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
 *   - Long-press reveals delete action (avoids permanent delete icon on every row)
 */
class HistoryActivity : ComponentActivity() {

    private val viewModel: ClipboardViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
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

    LaunchedEffect(Unit) { viewModel.loadItems() }

    LaunchedEffect(error) {
        val msg = error ?: return@LaunchedEffect
        snackbarHostState.showSnackbar(
            message = loadErrorTemplate.format(msg),
            actionLabel = dismissLabel,
        )
        viewModel.clearError()
    }

    Scaffold(
        modifier = modifier,
        containerColor = IdeBg,
        topBar = {
            // ── Compact IDE-style header (44 dp, matches macOS ViewShell h-11) ──
            TopAppBar(
                // Title uses titleLarge, which the theme overrides to 14 sp to
                // match the compact IDE bar (default Material titleLarge is 22 sp).
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
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor       = IdePanel,
                    titleContentColor    = IdeText,
                    actionIconContentColor = IdeDim,
                    navigationIconContentColor = IdeDim,
                ),
                // Constrain to 44 dp (matches macOS h-11 = 44 px) to keep the
                // header compact like a JetBrains IDE toolbar.
                modifier = Modifier.height(44.dp),
            )
        },
        snackbarHost = { SnackbarHost(hostState = snackbarHostState) },
    ) { innerPadding ->
        when {
            loading -> LoadingBox(innerPadding)
            items.isEmpty() -> EmptyState(innerPadding)
            else -> HistoryList(
                items = items,
                padding = innerPadding,
                onDelete = { id -> viewModel.deleteItem(id) },
            )
        }
    }
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
    onDelete: (String) -> Unit,
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
                onDelete = onDelete,
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
//   [type-icon]  [preview text ─────────────────────]  [timestamp]  [actions on expand]
//
// Long-press toggles the action row (delete / copy) to avoid cluttering every
// row with a permanent delete button.
// ─────────────────────────────────────────────────────────────────────────────

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun HistoryRow(
    item: ClipboardItem,
    maskSensitive: Boolean,
    onDelete: (String) -> Unit,
) {
    val detectedSensitive = remember(item.id, item.snippet) {
        if (item.isSensitive) true
        else try { isSensitive(item.snippet) } catch (_: UnsatisfiedLinkError) { false }
    }

    var expanded by remember(item.id) { mutableStateOf(false) }
    // Auto-collapse after 4 seconds of inactivity.
    LaunchedEffect(expanded) {
        if (expanded) {
            delay(4_000L)
            expanded = false
        }
    }

    val maskString = stringResource(R.string.sensitive_preview_mask)
    val display = when {
        detectedSensitive && maskSensitive -> maskString
        item.snippet.isBlank() -> stringResource(R.string.empty_history)
        else -> item.snippet
    }
    val rowBg = when {
        expanded           -> IdeSelection
        detectedSensitive  -> IdeDanger.copy(alpha = 0.07f)
        else               -> Color.Transparent
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(rowBg)
            .combinedClickable(
                onClick = { /* single tap: no-op on the row itself */ },
                onLongClick = { expanded = !expanded },
            )
            .padding(horizontal = 12.dp, vertical = 0.dp),
    ) {
        // ── Main row (always visible) ────────────────────────────────────
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .height(36.dp),           // compact 36 dp row (macOS ~28–34 px range)
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.Start,
        ) {
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
            if (!expanded) {
                Text(
                    text = formatTime(item.wallTimeMs),
                    style = MaterialTheme.typography.bodyMedium,
                    color = IdeFaint,
                    maxLines = 1,
                )
            }
        }

        // ── Action row (visible on long-press) ───────────────────────────
        if (expanded) {
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
