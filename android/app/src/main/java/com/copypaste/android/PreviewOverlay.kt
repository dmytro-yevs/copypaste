@file:OptIn(ExperimentalFoundationApi::class)

package com.copypaste.android

import android.graphics.BitmapFactory
import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.BookmarkAdded
import androidx.compose.material.icons.filled.OpenInNew
import androidx.compose.material.icons.filled.BookmarkBorder
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.SaveAlt
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.ui.theme.EaseOutExpo
import com.copypaste.android.ui.theme.IdeAccent
import com.copypaste.android.ui.theme.rememberReducedMotion
import com.copypaste.android.ui.theme.IdeAccentDim
import com.copypaste.android.ui.theme.IdeBorder
import com.copypaste.android.ui.theme.IdeDanger
import com.copypaste.android.ui.theme.IdeDangerDim
import com.copypaste.android.ui.theme.IdeDim
import com.copypaste.android.ui.theme.IdeElevated
import com.copypaste.android.ui.theme.IdeFaint
import com.copypaste.android.ui.theme.IdeInfo
import com.copypaste.android.ui.theme.IdeInfoDim
import com.copypaste.android.ui.theme.IdePanel
import com.copypaste.android.ui.theme.IdeText
import com.copypaste.android.ui.theme.IdeViolet
import com.copypaste.android.ui.theme.IdeVioletDim
import com.copypaste.android.ui.theme.IdeWarning
import com.copypaste.android.ui.theme.Motion
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

// ─────────────────────────────────────────────────────────────────────────────
// Preview peek phase — pure state machine
// ─────────────────────────────────────────────────────────────────────────────

/**
 * States for the long-press preview gesture.
 *
 * Idle    → [Peeking] on long-press hold
 * Peeking → [Pinned] when user drags UP ≥ [COMMIT_DRAG_THRESHOLD_DP] while held
 * Peeking → [Idle]   on release without enough upward drag
 * Pinned  → [Idle]   on explicit dismiss (scrim tap, Close button, BackHandler)
 */
sealed class PreviewPhase {
    object Idle    : PreviewPhase()
    object Peeking : PreviewPhase()
    object Pinned  : PreviewPhase()
}

/** Upward drag in dp required to commit peek → pinned. */
const val COMMIT_DRAG_THRESHOLD_DP = 64f

/**
 * Pure state-transition function — no Compose dependencies, easy to unit-test.
 */
fun nextPreviewPhase(
    current: PreviewPhase,
    dragUpDp: Float,
    released: Boolean,
): PreviewPhase = when (current) {
    PreviewPhase.Idle    -> current
    PreviewPhase.Peeking -> when {
        dragUpDp >= COMMIT_DRAG_THRESHOLD_DP -> PreviewPhase.Pinned
        released                             -> PreviewPhase.Idle
        else                                 -> PreviewPhase.Peeking
    }
    PreviewPhase.Pinned  -> current
}

// ─────────────────────────────────────────────────────────────────────────────
// Modifier — long-press peek gesture attached to each history row
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Attaches the long-press peek gesture to a composable.
 * No-op when [selectionMode] is true so multi-select is unaffected.
 */
@Composable
fun Modifier.previewPeekGesture(
    itemId: String,
    selectionMode: Boolean,
    onPeeking: (String) -> Unit,
    onPinned: (String) -> Unit,
    onDismissPeek: () -> Unit,
): Modifier {
    val haptic = LocalHapticFeedback.current
    if (selectionMode) return this
    return this.pointerInput(itemId) {
        var dragUpDp = 0f
        var phase: PreviewPhase = PreviewPhase.Idle
        detectDragGesturesAfterLongPress(
            onDragStart = { _ ->
                dragUpDp = 0f
                phase = PreviewPhase.Peeking
                haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                onPeeking(itemId)
            },
            onDrag = { change, dragAmount ->
                change.consume()
                val upDp = -dragAmount.y / density
                dragUpDp += upDp
                val next = nextPreviewPhase(phase, dragUpDp, released = false)
                if (next == PreviewPhase.Pinned && phase == PreviewPhase.Peeking) {
                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                    phase = next
                    onPinned(itemId)
                }
            },
            onDragEnd = {
                if (phase == PreviewPhase.Peeking) {
                    phase = PreviewPhase.Idle
                    onDismissPeek()
                }
            },
            onDragCancel = {
                if (phase == PreviewPhase.Peeking) {
                    phase = PreviewPhase.Idle
                    onDismissPeek()
                }
            },
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Full-screen preview overlay (in-tree Box — NOT a Dialog/Popup)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Full-screen overlay rendered as a sibling of the list inside the Scaffold
 * content Box. Two display modes driven by [phase]:
 *
 * [PreviewPhase.Peeking] — scrim + centered card, scale-in, "drag up to expand" hint.
 * [PreviewPhase.Pinned]  — scrim + card with scroll/zoom + action row.
 *
 * **Inset fix:** The outer Box applies [WindowInsets.statusBars] as top padding so
 * the card is never occluded by the status bar or the app TopAppBar on any device.
 * Additionally, the card uses `padding(top = topBarSafeTopDp)` to account for the
 * AppBar height when the overlay is drawn inside the Scaffold's content area.
 *
 * Pass [phase] == [PreviewPhase.Idle] to render nothing.
 */
@Composable
fun PreviewOverlay(
    phase: PreviewPhase,
    item: ClipboardItem?,
    repository: ClipboardRepository,
    settings: Settings,
    maskSensitive: Boolean,
    onDismiss: () -> Unit,
    onCopy: () -> Unit,
    onSetPinned: (Boolean) -> Unit,
    onDelete: () -> Unit,
    onSaveFile: () -> Unit,
    /** Open the file with the OS default application. Null when item is not a file. */
    onOpenFile: (() -> Unit)? = null,
) {
    if (phase == PreviewPhase.Idle || item == null) return

    val pinned = phase == PreviewPhase.Pinned
    // §8 a11y: suppress card scale-in when the user has requested reduced motion.
    val reducedMotion = rememberReducedMotion()

    // Dismiss pinned via system back
    BackHandler(enabled = pinned) { onDismiss() }

    // Scale-in animation for the card.  When reduced-motion is active the card
    // appears at full scale instantly instead of growing in from 0.85×.
    val cardScale by animateFloatAsState(
        targetValue = if (reducedMotion) 1f else if (phase == PreviewPhase.Idle) 0.85f else 1f,
        animationSpec = tween(durationMillis = if (reducedMotion) 0 else Motion.Base, easing = EaseOutExpo),
        label = "previewCardScale",
    )

    // Full-res text loaded lazily off main thread
    val fullTextState by produceState<String?>(
        initialValue = null,
        key1 = item.id,
        key2 = phase,
    ) {
        if (!item.isImage && !item.isFile && phase != PreviewPhase.Idle) {
            value = withContext(Dispatchers.IO) {
                runCatching {
                    repository.loadFullPlaintext(item.id, settings.encryptionKey)
                }.getOrNull()
            }
        }
    }

    // Full-res bitmap loaded lazily — decode at ≤1080px to bound memory
    val fullBitmapState by produceState<androidx.compose.ui.graphics.ImageBitmap?>(
        initialValue = null,
        key1 = item.id,
        key2 = phase,
    ) {
        if (item.isImage && phase != PreviewPhase.Idle) {
            value = withContext(Dispatchers.IO) {
                runCatching {
                    val bytes = repository.getImageBytes(item.id) ?: return@runCatching null
                    val opts = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                    BitmapFactory.decodeByteArray(bytes, 0, bytes.size, opts)
                    val targetPx = 1080
                    var sample = 1
                    while ((opts.outWidth / (sample * 2)) >= targetPx ||
                           (opts.outHeight / (sample * 2)) >= targetPx) {
                        sample *= 2
                    }
                    BitmapFactory.decodeByteArray(
                        bytes, 0, bytes.size,
                        BitmapFactory.Options().apply { inSampleSize = sample },
                    )?.asImageBitmap()
                }.getOrNull()
            }
        }
    }

    // Pinch-zoom + pan state
    var imageScale by rememberSaveable { mutableFloatStateOf(1f) }
    var imagePanX by rememberSaveable { mutableFloatStateOf(0f) }
    var imagePanY by rememberSaveable { mutableFloatStateOf(0f) }

    LaunchedEffect(phase) {
        if (phase == PreviewPhase.Peeking) {
            imageScale = 1f; imagePanX = 0f; imagePanY = 0f
        }
    }

    // ── Inset-aware top padding ───────────────────────────────────────────────
    // The overlay is placed inside Scaffold's content area, which starts below
    // the TopAppBar. But on devices where the Scaffold content Box is not
    // properly clamped (e.g. custom navigation bars, OEM themes), the card can
    // still be pushed behind the status bar. We explicitly add the status-bar
    // inset as additional top padding to the outer scrim box so the card is
    // always drawn in the visible region below the status bar.
    val statusBarTop = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()

    Box(
        modifier = Modifier
            .fillMaxSize()
            // Apply status-bar inset: card will never be drawn behind the status bar
            .padding(top = statusBarTop)
            .background(Color.Black.copy(alpha = if (pinned) 0.55f else 0.45f))
            .then(
                if (pinned) Modifier.clickable(
                    indication = null,
                    interactionSource = remember { MutableInteractionSource() },
                    onClick = onDismiss,
                ) else Modifier
            ),
        contentAlignment = Alignment.Center,
    ) {
        // Card — absorbs taps so scrim isn't triggered through the card
        Box(
            modifier = Modifier
                .widthIn(max = 560.dp)
                .heightIn(max = if (pinned) 700.dp else 480.dp)
                .padding(horizontal = 20.dp, vertical = if (pinned) 24.dp else 40.dp)
                .graphicsLayer {
                    scaleX = cardScale
                    scaleY = cardScale
                }
                .background(color = IdePanel, shape = RoundedCornerShape(16.dp))
                .clickable(
                    indication = null,
                    interactionSource = remember { MutableInteractionSource() },
                    onClick = {},
                ),
        ) {
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(16.dp),
            ) {
                PreviewHeader(
                    item = item,
                    pinned = pinned,
                    onDismiss = if (pinned) onDismiss else null,
                )
                HorizontalDivider(
                    modifier = Modifier.padding(vertical = 10.dp),
                    color = IdeBorder.copy(alpha = 0.5f),
                    thickness = 0.5.dp,
                )
                Box(modifier = Modifier.weight(1f)) {
                    when {
                        item.isImage -> PreviewImageContent(
                            bitmap = fullBitmapState,
                            pinned = pinned,
                            imageScale = imageScale,
                            imagePanX = imagePanX,
                            imagePanY = imagePanY,
                            onTransform = { newScale, panDelta ->
                                imageScale = (imageScale * newScale).coerceIn(1f, 5f)
                                if (imageScale > 1f) {
                                    imagePanX += panDelta.x
                                    imagePanY += panDelta.y
                                } else {
                                    imagePanX = 0f; imagePanY = 0f
                                }
                            },
                        )
                        item.isFile  -> PreviewFileContent(item = item)
                        else         -> PreviewTextContent(
                            item = item,
                            fullText = fullTextState,
                            maskSensitive = maskSensitive,
                            pinned = pinned,
                        )
                    }
                }
                if (!pinned) {
                    HorizontalDivider(
                        modifier = Modifier.padding(vertical = 8.dp),
                        color = IdeBorder.copy(alpha = 0.3f),
                        thickness = 0.5.dp,
                    )
                    Text(
                        text = stringResource(R.string.preview_drag_up_hint),
                        style = MaterialTheme.typography.labelSmall,
                        color = IdeFaint,
                        modifier = Modifier.align(Alignment.CenterHorizontally),
                    )
                } else {
                    HorizontalDivider(
                        modifier = Modifier.padding(vertical = 8.dp),
                        color = IdeBorder.copy(alpha = 0.5f),
                        thickness = 0.5.dp,
                    )
                    PreviewActionRow(
                        item = item,
                        onCopy = onCopy,
                        onSetPinned = onSetPinned,
                        onDelete = onDelete,
                        onSaveFile = if (item.isFile) onSaveFile else null,
                        onOpenFile = if (item.isFile) onOpenFile else null,
                    )
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Header
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun PreviewHeader(
    item: ClipboardItem,
    pinned: Boolean,
    onDismiss: (() -> Unit)?,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            PreviewContentTypeChip(item.contentType, item.isSensitive)
            item.sourceApp?.let { pkg ->
                sourceAppLabel(pkg)?.let { label ->
                    Text(
                        text = label,
                        style = MaterialTheme.typography.labelSmall,
                        color = IdeFaint,
                    )
                }
            }
        }
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text(
                text = relativeTimePreview(item.wallTimeMs),
                style = TextStyle(
                    fontSize = 11.sp,
                    fontWeight = FontWeight.Normal,
                    fontFeatureSettings = "tnum",
                ),
                color = IdeFaint,
            )
            if (pinned && onDismiss != null) {
                IconButton(onClick = onDismiss, modifier = Modifier.size(28.dp)) {
                    Icon(
                        imageVector = Icons.Filled.Close,
                        contentDescription = stringResource(R.string.cd_close_selection),
                        tint = IdeDim,
                        modifier = Modifier.size(16.dp),
                    )
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Content-type chip (copy with full visibility — no internal visibility)
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun PreviewContentTypeChip(contentType: String, isSensitive: Boolean) {
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
            ),
            color = fg,
            maxLines = 1,
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Text content
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun PreviewTextContent(
    item: ClipboardItem,
    fullText: String?,
    maskSensitive: Boolean,
    pinned: Boolean,
) {
    val masked = item.isSensitive && maskSensitive
    val displayText = when {
        masked       -> "•••••••••••••"
        fullText != null -> fullText
        else         -> item.snippet
    }

    if (pinned && !masked) {
        SelectionContainer {
            Text(
                text = displayText,
                style = MaterialTheme.typography.bodyMedium,
                color = IdeText,
                modifier = Modifier
                    .fillMaxSize()
                    .verticalScroll(rememberScrollState()),
            )
        }
    } else {
        Text(
            text = displayText,
            style = MaterialTheme.typography.bodyMedium,
            color = if (masked) IdeDim else IdeText,
            maxLines = if (pinned) Int.MAX_VALUE else 8,
            overflow = TextOverflow.Clip,
            modifier = Modifier.fillMaxSize(),
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Image content
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun PreviewImageContent(
    bitmap: androidx.compose.ui.graphics.ImageBitmap?,
    pinned: Boolean,
    imageScale: Float,
    imagePanX: Float,
    imagePanY: Float,
    onTransform: (scaleChange: Float, panDelta: Offset) -> Unit,
) {
    Box(
        modifier = Modifier
            .fillMaxSize()
            .then(
                if (pinned) Modifier.pointerInput(Unit) {
                    detectTransformGestures { _, pan, zoom, _ ->
                        onTransform(zoom, pan)
                    }
                } else Modifier
            ),
        contentAlignment = Alignment.Center,
    ) {
        if (bitmap != null) {
            Image(
                bitmap = bitmap,
                contentDescription = null,
                contentScale = ContentScale.Fit,
                modifier = Modifier
                    .fillMaxSize()
                    .graphicsLayer {
                        scaleX = imageScale
                        scaleY = imageScale
                        translationX = if (imageScale > 1f) imagePanX else 0f
                        translationY = if (imageScale > 1f) imagePanY else 0f
                    },
            )
        } else {
            CircularProgressIndicator(
                color = IdeAccent,
                strokeWidth = 2.dp,
                modifier = Modifier.size(24.dp),
            )
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// File content
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun PreviewFileContent(item: ClipboardItem) {
    Column(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(
            imageVector = Icons.Filled.AttachFile,
            contentDescription = null,
            tint = IdeDim,
            modifier = Modifier.size(40.dp),
        )
        Spacer(Modifier.size(12.dp))
        Text(
            text = item.snippet,
            style = MaterialTheme.typography.bodyLarge,
            color = IdeText,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Action row (pinned mode)
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun PreviewActionRow(
    item: ClipboardItem,
    onCopy: () -> Unit,
    onSetPinned: (Boolean) -> Unit,
    onDelete: () -> Unit,
    onSaveFile: (() -> Unit)?,
    /** Open with default app. Non-null only for file items. */
    onOpenFile: (() -> Unit)? = null,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.End,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onCopy, modifier = Modifier.size(36.dp)) {
            Icon(
                imageVector = Icons.Filled.ContentCopy,
                contentDescription = stringResource(R.string.cd_copy),
                tint = IdeAccent,
                modifier = Modifier.size(18.dp),
            )
        }
        Spacer(Modifier.width(4.dp))
        IconButton(onClick = { onSetPinned(!item.pinned) }, modifier = Modifier.size(36.dp)) {
            Icon(
                imageVector = if (item.pinned) Icons.Filled.BookmarkAdded
                              else Icons.Filled.BookmarkBorder,
                contentDescription = stringResource(
                    if (item.pinned) R.string.action_unpin else R.string.action_pin,
                ),
                tint = if (item.pinned) IdeWarning else IdeDim,
                modifier = Modifier.size(18.dp),
            )
        }
        // Open with default app — shown only for file items
        if (onOpenFile != null) {
            Spacer(Modifier.width(4.dp))
            IconButton(onClick = onOpenFile, modifier = Modifier.size(36.dp)) {
                Icon(
                    imageVector = Icons.Filled.OpenInNew,
                    contentDescription = stringResource(R.string.cd_open_file),
                    tint = IdeAccent,
                    modifier = Modifier.size(18.dp),
                )
            }
        }
        if (onSaveFile != null) {
            Spacer(Modifier.width(4.dp))
            IconButton(onClick = onSaveFile, modifier = Modifier.size(36.dp)) {
                Icon(
                    imageVector = Icons.Filled.SaveAlt,
                    contentDescription = stringResource(R.string.action_save_file),
                    tint = IdeAccent,
                    modifier = Modifier.size(18.dp),
                )
            }
        }
        Spacer(Modifier.width(4.dp))
        IconButton(onClick = onDelete, modifier = Modifier.size(36.dp)) {
            Icon(
                imageVector = Icons.Filled.Delete,
                contentDescription = stringResource(R.string.cd_delete),
                tint = IdeDanger,
                modifier = Modifier.size(18.dp),
            )
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/** Relative-time helper for PreviewOverlay (no dependency on HistoryActivity internals). */
internal fun relativeTimePreview(ms: Long): String {
    if (ms <= 0L) return "—"
    val diff = System.currentTimeMillis() - ms
    return when {
        diff < 60_000L         -> "just now"
        diff < 3_600_000L      -> "${diff / 60_000}m ago"
        diff < 86_400_000L     -> "${diff / 3_600_000}h ago"
        diff < 7 * 86_400_000L -> "${diff / 86_400_000}d ago"
        else -> java.text.DateFormat
            .getDateInstance(java.text.DateFormat.SHORT)
            .format(java.util.Date(ms))
    }
}
