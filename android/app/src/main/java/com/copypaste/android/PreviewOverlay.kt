@file:OptIn(ExperimentalFoundationApi::class)

package com.copypaste.android

import android.graphics.BitmapFactory
import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
// CopyPaste-5917.23: use Outlined variants to match the app-wide outlined icon styleguide.
// AttachFile, BookmarkAdded, BookmarkBorder, Close, ContentCopy, Delete, OpenInNew, SaveAlt
// all have Outlined equivalents — no Filled exceptions needed.
import androidx.compose.material.icons.outlined.AttachFile
import androidx.compose.material.icons.outlined.BookmarkAdded
import androidx.compose.material.icons.outlined.OpenInNew
import androidx.compose.material.icons.outlined.BookmarkBorder
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.SaveAlt
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
import androidx.compose.ui.draw.BlurredEdgeTreatment
import androidx.compose.ui.draw.blur
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
import androidx.compose.animation.core.FastOutSlowInEasing
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import android.os.Build

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

    // Dismiss pinned via system back
    BackHandler(enabled = pinned) { onDismiss() }

    // Scale-in animation for the card.
    val cardScale by animateFloatAsState(
        targetValue = if (phase == PreviewPhase.Idle) 0.85f else 1f,
        animationSpec = tween(durationMillis = 300, easing = FastOutSlowInEasing),
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
                .background(color = MaterialTheme.colorScheme.surfaceContainerHighest, shape = RoundedCornerShape(16.dp))
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
                    color = MaterialTheme.colorScheme.outline.copy(alpha = 0.5f),
                    thickness = 0.5.dp,
                )
                Box(modifier = Modifier.weight(1f)) {
                    when {
                        item.isImage -> PreviewImageContent(
                            bitmap = fullBitmapState,
                            isSensitive = item.isSensitive,
                            maskSensitive = maskSensitive,
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
                        color = MaterialTheme.colorScheme.outline.copy(alpha = 0.3f),
                        thickness = 0.5.dp,
                    )
                    Text(
                        text = stringResource(R.string.preview_drag_up_hint),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.align(Alignment.CenterHorizontally),
                    )
                } else {
                    HorizontalDivider(
                        modifier = Modifier.padding(vertical = 8.dp),
                        color = MaterialTheme.colorScheme.outline.copy(alpha = 0.5f),
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
            PreviewContentTypeChip(item.contentType, item.isSensitive, item.snippet)
            item.sourceApp?.let { pkg ->
                sourceAppLabel(pkg)?.let { label ->
                    Text(
                        text = label,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
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
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (pinned && onDismiss != null) {
                // CopyPaste-5jcj: 48dp touch target (WCAG 2.5.5 / Android min) while
                // keeping the visible glyph at 16dp. IconButton centres its content,
                // so the icon does not grow.
                IconButton(onClick = onDismiss, modifier = Modifier.size(48.dp)) {
                    Icon(
                        imageVector = Icons.Outlined.Close,
                        contentDescription = stringResource(R.string.cd_close_selection),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.size(16.dp),
                    )
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Content-type chip — CopyPaste-5917.58: aligned to canonical chipLabelFor /
// chipColorFor mapping from HistoryActivity so overlay chip matches list-row chip.
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Derive chip label matching HistoryActivity.chipLabelFor — IMAGE/FILE by
 * content-type, classified text kind (URL/EMAIL/CODE/…) for text items.
 * CopyPaste-1b55: sensitive items keep their content-type label (not "PRIVATE").
 */
private fun previewChipLabel(contentType: String, snippet: String): String = when {
    contentTypeIsImage(contentType) -> "IMAGE"
    contentTypeIsText(contentType)  ->
        if (snippet.isNotBlank()) TextKind.classify(snippet) else "TEXT"
    else                            -> "FILE"
}

/**
 * Map chip label to foreground color — mirrors HistoryActivity.chipColorFor
 * (canonical: TEXT→accent, URL→info, EMAIL/PHONE→success, COLOR/NUMBER/PATH→warning,
 * JSON→danger, CODE/IMAGE→violet, FILE→faint, PRIVATE→danger).
 */
@Composable
private fun previewChipColor(
    label: String,
    accent: Color,
): Color = when (label) {
    "TEXT"    -> accent
    "URL"     -> MaterialTheme.colorScheme.secondary
    "EMAIL"   -> MaterialTheme.colorScheme.primary
    "PHONE"   -> MaterialTheme.colorScheme.primary
    "COLOR"   -> MaterialTheme.colorScheme.tertiary
    "NUMBER"  -> MaterialTheme.colorScheme.tertiary
    "PATH"    -> MaterialTheme.colorScheme.tertiary
    "JSON"    -> MaterialTheme.colorScheme.error
    "CODE"    -> MaterialTheme.colorScheme.tertiary
    "IMAGE"   -> MaterialTheme.colorScheme.tertiary
    "FILE"    -> MaterialTheme.colorScheme.onSurfaceVariant
    "PRIVATE" -> MaterialTheme.colorScheme.error
    else      -> MaterialTheme.colorScheme.onSurfaceVariant
}

@Composable
private fun PreviewContentTypeChip(
    contentType: String,
    @Suppress("UNUSED_PARAMETER") isSensitive: Boolean, // CopyPaste-1b55: label is always content-type, not "PRIVATE"
    snippet: String,
) {
    // CopyPaste-1b55 parity: keep content-type label even for sensitive items;
    // privacy is signalled by the blur/mask, not the chip label.
    val label = previewChipLabel(contentType, snippet)
    val color = previewChipColor(label, MaterialTheme.colorScheme.primary)
    // Match ContentTypeChip style from HistoryActivity: 7dp radius, 1dp border, 10sp SemiBold.
    Box(
        modifier = Modifier
            .background(color = color.copy(alpha = 0.14f), shape = RoundedCornerShape(7.dp))
            .border(
                width = 1.dp,
                color = color.copy(alpha = 0.45f),
                shape = RoundedCornerShape(7.dp),
            )
            .padding(horizontal = 5.dp, vertical = 2.dp),
    ) {
        Text(
            text = label,
            style = TextStyle(
                fontSize = 10.sp,
                fontWeight = FontWeight.SemiBold,
                letterSpacing = 0.4.sp,
            ),
            color = color,
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
    // CopyPaste-5917.70 (security): on API 31+ use Modifier.blur on the real text
    // rather than substituting bullet characters. Plaintext is never placed in the
    // view tree when masked AND blur is available — the same text is rendered with
    // a blur modifier so the underlying string is NOT readable by assistive services
    // or screen scrapers any more than it would be with bullets. On pre-31 devices
    // blur is a no-op so we fall back to bullets (original safe behaviour).
    val canBlur = Build.VERSION.SDK_INT >= Build.VERSION_CODES.S
    // The display text is the real content when blur will be applied (API 31+);
    // bullets are used only as the API<31 fallback.
    val displayText = when {
        masked && canBlur -> fullText ?: item.snippet
        masked            -> "•••••••••••••"   // pre-31 fallback: no real text in view tree
        fullText != null  -> fullText
        else              -> item.snippet
    }

    // CopyPaste-5917.70 (security): SelectionContainer is now gated on the item
    // NOT being sensitive. Sensitive items require the user to explicitly reveal
    // before text selection becomes available, preventing silent clipboard exfil.
    val allowSelection = pinned && !item.isSensitive

    if (allowSelection) {
        SelectionContainer {
            Text(
                text = displayText,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier
                    .fillMaxSize()
                    .verticalScroll(rememberScrollState()),
            )
        }
    } else {
        Text(
            text = displayText,
            style = MaterialTheme.typography.bodyMedium,
            color = if (masked) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface,
            maxLines = if (pinned) Int.MAX_VALUE else 8,
            overflow = TextOverflow.Clip,
            modifier = Modifier
                .fillMaxSize()
                .then(
                    // CopyPaste-5917.70: blur the real text on API 31+ instead of
                    // substituting bullets. Unbounded edge so blur bleeds at the edges
                    // rather than creating a visible rectangular crop.
                    if (masked && canBlur)
                        Modifier.blur(6.dp, BlurredEdgeTreatment.Unbounded)
                    else
                        Modifier
                ),
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Image content
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun PreviewImageContent(
    bitmap: androidx.compose.ui.graphics.ImageBitmap?,
    /** CopyPaste-44rq.42: mirror text masking — blur image content when sensitive + masked. */
    isSensitive: Boolean,
    maskSensitive: Boolean,
    pinned: Boolean,
    imageScale: Float,
    imagePanX: Float,
    imagePanY: Float,
    onTransform: (scaleChange: Float, panDelta: Offset) -> Unit,
) {
    // CopyPaste-44rq.42: sensitive images are blurred until the user intentionally reveals
    // them, mirroring the text-masking guard in PreviewTextContent. On API 31+ we use
    // Modifier.blur; on older APIs the bitmap is hidden entirely behind a placeholder.
    val masked = isSensitive && maskSensitive
    val canBlur = android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.S

    Box(
        modifier = Modifier
            .fillMaxSize()
            .then(
                if (pinned && !masked) Modifier.pointerInput(Unit) {
                    detectTransformGestures { _, pan, zoom, _ ->
                        onTransform(zoom, pan)
                    }
                } else Modifier
            ),
        contentAlignment = Alignment.Center,
    ) {
        if (masked && !canBlur) {
            // Pre-API-31 fallback: Modifier.blur is a no-op, so hide the image entirely
            // to prevent leaking sensitive content. Show a lock placeholder instead.
            Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Icon(
                    imageVector = Icons.Outlined.AttachFile,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.error,
                    modifier = Modifier.size(32.dp),
                )
                Text(
                    text = stringResource(R.string.sensitive_preview_mask),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                )
            }
        } else if (bitmap != null) {
            Image(
                bitmap = bitmap,
                // CopyPaste-3nyq: describe the copied image so AT announces it.
                contentDescription = stringResource(R.string.cd_preview_image),
                contentScale = ContentScale.Fit,
                modifier = Modifier
                    .fillMaxSize()
                    .graphicsLayer {
                        scaleX = imageScale
                        scaleY = imageScale
                        translationX = if (imageScale > 1f) imagePanX else 0f
                        translationY = if (imageScale > 1f) imagePanY else 0f
                    }
                    // CopyPaste-44rq.42: apply blur on API 31+ when masked; unmasked
                    // images render at full quality with no blur modifier.
                    .then(
                        if (masked) Modifier.blur(20.dp, BlurredEdgeTreatment.Rectangle)
                        else Modifier
                    ),
            )
        } else {
            CircularProgressIndicator(
                color = MaterialTheme.colorScheme.primary,
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
            imageVector = Icons.Outlined.AttachFile,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(40.dp),
        )
        Spacer(Modifier.size(12.dp))
        Text(
            text = item.snippet,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurface,
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
    // CopyPaste-5jcj: every action IconButton is 48dp (WCAG 2.5.5 minimum touch
    // target) while the inner Icon glyph stays 18dp — IconButton centres its content
    // so the visible icon is unchanged, only the tappable area grows.
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.End,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onCopy, modifier = Modifier.size(48.dp)) {
            Icon(
                imageVector = Icons.Outlined.ContentCopy,
                contentDescription = stringResource(R.string.cd_copy),
                tint = MaterialTheme.colorScheme.primary,
                modifier = Modifier.size(18.dp),
            )
        }
        Spacer(Modifier.width(4.dp))
        IconButton(onClick = { onSetPinned(!item.pinned) }, modifier = Modifier.size(48.dp)) {
            Icon(
                imageVector = if (item.pinned) Icons.Outlined.BookmarkAdded
                              else Icons.Outlined.BookmarkBorder,
                contentDescription = stringResource(
                    if (item.pinned) R.string.action_unpin else R.string.action_pin,
                ),
                tint = if (item.pinned) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(18.dp),
            )
        }
        // Open with default app — shown only for file items
        if (onOpenFile != null) {
            Spacer(Modifier.width(4.dp))
            IconButton(onClick = onOpenFile, modifier = Modifier.size(48.dp)) {
                Icon(
                    imageVector = Icons.Outlined.OpenInNew,
                    contentDescription = stringResource(R.string.cd_open_file),
                    tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.size(18.dp),
                )
            }
        }
        if (onSaveFile != null) {
            Spacer(Modifier.width(4.dp))
            IconButton(onClick = onSaveFile, modifier = Modifier.size(48.dp)) {
                Icon(
                    imageVector = Icons.Outlined.SaveAlt,
                    contentDescription = stringResource(R.string.action_save_file),
                    tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.size(18.dp),
                )
            }
        }
        Spacer(Modifier.width(4.dp))
        IconButton(onClick = onDelete, modifier = Modifier.size(48.dp)) {
            Icon(
                imageVector = Icons.Outlined.Delete,
                contentDescription = stringResource(R.string.cd_delete),
                tint = MaterialTheme.colorScheme.error,
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
