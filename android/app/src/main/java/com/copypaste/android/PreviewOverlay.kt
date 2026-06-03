@file:OptIn(ExperimentalFoundationApi::class)

package com.copypaste.android

import android.graphics.Bitmap
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
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.BookmarkAdded
import androidx.compose.material.icons.filled.BookmarkBorder
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.SaveAlt
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.mutableStateOf
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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.ui.theme.EaseOutExpo
import com.copypaste.android.ui.theme.IdeAccent
import com.copypaste.android.ui.theme.IdeBorder
import com.copypaste.android.ui.theme.IdeDanger
import com.copypaste.android.ui.theme.IdeDim
import com.copypaste.android.ui.theme.IdeFaint
import com.copypaste.android.ui.theme.IdePanel
import com.copypaste.android.ui.theme.IdeText
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
 * Idle          → [Peeking] on long-press hold
 * Peeking       → [Pinned] when user drags UP ≥ [COMMIT_DRAG_THRESHOLD_DP] while held
 * Peeking       → [Idle]   on release without enough upward drag
 * Pinned        → [Idle]   on explicit dismiss (scrim tap, Close button, BackHandler)
 */
sealed class PreviewPhase {
    object Idle    : PreviewPhase()
    object Peeking : PreviewPhase()
    object Pinned  : PreviewPhase()
}

/**
 * Threshold (in dp) of upward drag required while the long-press is held to
 * commit the preview to pinned state.
 */
const val COMMIT_DRAG_THRESHOLD_DP = 64f

/**
 * Pure state-transition function — no Compose dependencies, easy to unit-test.
 *
 * @param current   current [PreviewPhase]
 * @param dragUpDp  cumulative upward drag in dp (positive = moved upward)
 * @param released  true when the pointer was released
 * @return the next [PreviewPhase]
 */
fun nextPreviewPhase(
    current: PreviewPhase,
    dragUpDp: Float,
    released: Boolean,
): PreviewPhase = when (current) {
    PreviewPhase.Idle    -> current  // only onDragStart → Peeking from gesture handler
    PreviewPhase.Peeking -> when {
        dragUpDp >= COMMIT_DRAG_THRESHOLD_DP -> PreviewPhase.Pinned
        released                             -> PreviewPhase.Idle
        else                                 -> PreviewPhase.Peeking
    }
    PreviewPhase.Pinned  -> current  // only explicit dismiss → Idle
}

// ─────────────────────────────────────────────────────────────────────────────
// Modifier — long-press peek gesture attached to each history row
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Attaches the long-press peek gesture to a composable.
 *
 * When [selectionMode] is true the modifier is a no-op so the row falls back to
 * the existing combinedClickable selection behaviour.
 *
 * @param itemId        stable key for the item driving this gesture
 * @param selectionMode when true the gesture is disabled
 * @param onPeeking     called with the item id when the long-press hold starts
 * @param onPinned      called with the item id when the drag-up commit fires
 * @param onDismissPeek called when a plain release without commit ends the peek
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
                // Upward drag: negative Y in Compose coordinates → positive dragUpDp.
                val upPx = -dragAmount.y
                val upDp = upPx / density
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
                // If already Pinned we leave it pinned; explicit dismiss handles it.
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
 * content Box.  Two display modes driven by [phase]:
 *
 * [PreviewPhase.Peeking] — scrim + centered card, scale-in, "drag up to expand" hint.
 * [PreviewPhase.Pinned]  — scrim + card with scroll/zoom + action row.
 *
 * Pass [phase] == [PreviewPhase.Idle] to not render anything.
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
) {
    if (phase == PreviewPhase.Idle || item == null) return

    val pinned = phase == PreviewPhase.Pinned

    // Dismiss pinned via BackHandler
    BackHandler(enabled = pinned) { onDismiss() }

    // Animated scale-in for the card
    val scaleTarget = if (phase == PreviewPhase.Idle) 0.85f else 1f
    val cardScale by animateFloatAsState(
        targetValue = scaleTarget,
        animationSpec = tween(durationMillis = Motion.Base, easing = EaseOutExpo),
        label = "previewCardScale",
    )

    // Full-res content loaded lazily on Dispatchers.IO, only when overlay opens.
    // Released (set to null) on dismiss via key = item.id + phase so if the phase
    // returns to Idle the produceState restarts and previous heavy data is GC'd.
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

    val fullBitmapState by produceState<androidx.compose.ui.graphics.ImageBitmap?>(
        initialValue = null,
        key1 = item.id,
        key2 = phase,
    ) {
        if (item.isImage && phase != PreviewPhase.Idle) {
            value = withContext(Dispatchers.IO) {
                runCatching {
                    // Decode at display-box size (max 1080px on the long edge) to
                    // bound memory — never load full 1:1 resolution.
                    val bytes = repository.getImageBytes(item.id) ?: return@runCatching null
                    val opts = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                    BitmapFactory.decodeByteArray(bytes, 0, bytes.size, opts)
                    val rawW = opts.outWidth.coerceAtLeast(1)
                    val rawH = opts.outHeight.coerceAtLeast(1)
                    val targetPx = 1080
                    var sample = 1
                    while ((rawW / (sample * 2)) >= targetPx || (rawH / (sample * 2)) >= targetPx) {
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

    // Dismiss if the previewed item no longer exists in the list — handled by
    // the parent via the `items` list; parent calls onDismiss when item is null.

    // Pinch-zoom + pan state for the pinned image view
    var imageScale by rememberSaveable { mutableFloatStateOf(1f) }
    var imagePanX by rememberSaveable { mutableFloatStateOf(0f) }
    var imagePanY by rememberSaveable { mutableFloatStateOf(0f) }

    // Reset zoom when we transition to Peeking (fresh open)
    LaunchedEffect(phase) {
        if (phase == PreviewPhase.Peeking) {
            imageScale = 1f
            imagePanX = 0f
            imagePanY = 0f
        }
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            // Scrim: dismiss on tap only when pinned (while peeking the gesture
            // stream is still owned by the row's pointerInput).
            .background(Color.Black.copy(alpha = if (pinned) 0.55f else 0.45f))
            .then(
                if (pinned) Modifier.clickable(
                    indication = null,
                    interactionSource = remember { androidx.compose.foundation.interaction.MutableInteractionSource() },
                    onClick = onDismiss,
                ) else Modifier
            ),
        contentAlignment = Alignment.Center,
    ) {
        // Card — stops click propagation to scrim
        Box(
            modifier = Modifier
                .widthIn(max = 560.dp)
                .heightIn(max = if (pinned) 700.dp else 480.dp)
                .padding(horizontal = 20.dp, vertical = if (pinned) 24.dp else 40.dp)
                .graphicsLayer {
                    scaleX = cardScale
                    scaleY = cardScale
                }
                .background(
                    color = IdePanel,
                    shape = RoundedCornerShape(16.dp),
                )
                .clickable(
                    indication = null,
                    interactionSource = remember { androidx.compose.foundation.interaction.MutableInteractionSource() },
                    onClick = {}, // absorb taps so scrim isn't triggered through card
                ),
        ) {
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(16.dp),
            ) {
                // ── Header ───────────────────────────────────────────────────
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

                // ── Content area ─────────────────────────────────────────────
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
                                    imagePanX = 0f
                                    imagePanY = 0f
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
                    // ── Peek hint ────────────────────────────────────────────
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
                    // ── Action row (pinned only) ──────────────────────────────
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
                    )
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Header — ContentTypeChip + source + relative time
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
            PreviewContentTypeChip(
                contentType = item.contentType,
                isSensitive = item.isSensitive,
            )
            item.sourceApp?.let { pkg ->
                val label = sourceAppLabel(pkg)
                if (label != null) {
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
                text = relativeTimePublic(item.wallTimeMs),
                style = TextStyle(
                    fontSize = 11.sp,
                    fontWeight = FontWeight.Normal,
                    fontFeatureSettings = "tnum",
                ),
                color = IdeFaint,
            )
            if (pinned && onDismiss != null) {
                IconButton(
                    onClick = onDismiss,
                    modifier = Modifier.size(28.dp),
                ) {
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
// Content-type chip (copy of HistoryActivity's private chip, accessible here)
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun PreviewContentTypeChip(contentType: String, isSensitive: Boolean) {
    val (label, fg, bg) = when {
        isSensitive -> Triple("PRIVATE", com.copypaste.android.ui.theme.IdeDanger, com.copypaste.android.ui.theme.IdeDangerDim)
        contentType.startsWith("image/") || contentType == "image" ->
            Triple("IMG", com.copypaste.android.ui.theme.IdeViolet, com.copypaste.android.ui.theme.IdeVioletDim)
        contentType == "url" || contentType.startsWith("url") ->
            Triple("URL", com.copypaste.android.ui.theme.IdeInfo, com.copypaste.android.ui.theme.IdeInfoDim)
        contentType == "text" || contentType.startsWith("text/") ->
            Triple("TEXT", IdeAccent, com.copypaste.android.ui.theme.IdeAccentDim)
        else -> Triple("FILE", IdeDim, com.copypaste.android.ui.theme.IdeElevated)
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
        masked    -> "•••••••••••••"
        fullText != null -> fullText
        else      -> item.snippet
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
            overflow = androidx.compose.ui.text.style.TextOverflow.Clip,
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
            // Loading placeholder
            androidx.compose.material3.CircularProgressIndicator(
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
            overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
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
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.End,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // Copy
        IconButton(onClick = onCopy, modifier = Modifier.size(36.dp)) {
            Icon(
                imageVector = Icons.Filled.ContentCopy,
                contentDescription = stringResource(R.string.cd_copy),
                tint = IdeAccent,
                modifier = Modifier.size(18.dp),
            )
        }
        Spacer(Modifier.width(4.dp))
        // Pin / unpin
        IconButton(
            onClick = { onSetPinned(!item.pinned) },
            modifier = Modifier.size(36.dp),
        ) {
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
        // Save file (only shown for file items)
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
        // Delete
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

/**
 * Public wrapper around the private relativeTime function in HistoryActivity.
 * Duplicated here so PreviewOverlay.kt has no internal visibility dependency.
 */
internal fun relativeTimePublic(ms: Long): String {
    if (ms <= 0L) return "—"
    val diff = System.currentTimeMillis() - ms
    return when {
        diff < 60_000L         -> "just now"
        diff < 3_600_000L      -> "${diff / 60_000}m ago"
        diff < 86_400_000L     -> "${diff / 3_600_000}h ago"
        diff < 7 * 86_400_000L -> "${diff / 86_400_000}d ago"
        else                   -> java.text.DateFormat
            .getDateInstance(java.text.DateFormat.SHORT)
            .format(java.util.Date(ms))
    }
}
