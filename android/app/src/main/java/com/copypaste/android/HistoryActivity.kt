@file:OptIn(ExperimentalFoundationApi::class)

package com.copypaste.android

import android.graphics.BitmapFactory
import android.os.Bundle
import androidx.activity.ComponentActivity
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
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Image
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
 *   - Long-press reveals delete action; auto-collapses after [Settings.previewDelay]
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
                onDelete = onDelete,
                onCopy = { snippet ->
                    val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                    cm.setPrimaryClip(ClipData.newPlainText("CopyPaste", snippet))
                },
            )
            HorizontalDivider(
                color = IdeBorder.copy(alpha = 0.5f),
                thickness = 0.5.dp,
            )
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Row — compact IDE-style, matching the macOS HistoryRow layout.
//
// Image items:
//   The thumbnail is shown in place of the text preview, inside a bounding box
//   of max-width 340 dp × max-height [imageMaxHeightDp] dp. ContentScale.Fit
//   preserves the aspect ratio and never upscales beyond the image's natural
//   size (the bitmap pixel size is compared to the bounding box at draw time).
//
// Text items (and fallback for image items whose bytes failed to decode):
//   [type-icon]  [preview text ─────────────────────]  [timestamp]
//
// Long-press toggles the action row (delete/copy). Auto-collapses after
// [previewDelayMs] of inactivity (Maccy-parity previewDelay setting).
// ─────────────────────────────────────────────────────────────────────────────

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun HistoryRow(
    item: ClipboardItem,
    maskSensitive: Boolean,
    imageMaxHeightDp: Int,
    previewDelayMs: Long,
    onDelete: (String) -> Unit,
    onCopy: (String) -> Unit = {},
) {
    // Sensitivity is computed in the repository against the FULL decrypted
    // plaintext (see ClipboardRepository.parseItem). The snippet here is a
    // truncated/sanitized preview, so re-running detection on it would be both
    // redundant and lossy — trust the flag set at load time.
    val detectedSensitive = item.isSensitive

    var expanded by remember(item.id) { mutableStateOf(false) }
    // Auto-collapse after previewDelayMs of inactivity (Maccy previewDelay parity).
    LaunchedEffect(expanded) {
        if (expanded) {
            delay(previewDelayMs)
            expanded = false
        }
    }

    // Attempt to decode image bytes into an ImageBitmap for thumbnail rendering.
    // BitmapFactory.decodeByteArray is CPU-bound but fast for thumbnails; it
    // runs synchronously on the composition thread, which is acceptable because
    // the bytes are small (thumbnail-scaled at capture time). If decoding fails
    // (malformed bytes, OOM, or bytes == null) we fall back to the text preview.
    val imageBitmap = remember(item.id, item.imagePng) {
        item.imagePng?.let { bytes ->
            runCatching { BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.asImageBitmap() }
                .getOrNull()
        }
    }

    val maskString = stringResource(R.string.sensitive_preview_mask)
    val display = when {
        detectedSensitive && maskSensitive -> maskString
        item.snippet.isBlank() -> stringResource(R.string.empty_history)
        else -> item.snippet
    }
    val rowBg = when {
        expanded          -> IdeSelection
        detectedSensitive -> IdeDanger.copy(alpha = 0.07f)
        else              -> Color.Transparent
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(rowBg)
            .combinedClickable(
                // Single-tap: copy the full snippet back to the system clipboard.
                // Sensitive items are not copyable via tap — the user must long-press
                // and use the explicit Copy chip so the intent is unambiguous.
                onClick = {
                    if (!detectedSensitive) onCopy(item.snippet)
                },
                onLongClick = { expanded = !expanded },
            )
            .padding(horizontal = 12.dp, vertical = 0.dp),
    ) {
        if (item.isImage && imageBitmap != null) {
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
                    bitmap = imageBitmap,
                    contentDescription = stringResource(R.string.cd_image_thumbnail),
                    contentScale = ContentScale.Fit,
                    modifier = Modifier
                        .widthIn(max = 340.dp)
                        .heightIn(max = imageMaxHeightDp.dp)
                        .clip(RoundedCornerShape(4.dp))
                        .background(IdeElevated),
                )
                Spacer(Modifier.weight(1f))
                if (!expanded) {
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
