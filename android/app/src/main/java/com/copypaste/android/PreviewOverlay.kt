package com.copypaste.android

import android.graphics.BitmapFactory
import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.animation.core.FastOutSlowInEasing
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

// ─────────────────────────────────────────────────────────────────────────────
// Full-screen preview overlay (in-tree Box — NOT a Dialog/Popup)
//
// CopyPaste-vp63.42: split into focused files — this file now holds only the
// overlay host composable (scrim, card chrome, lazy full-res content loading,
// pinch-zoom/pan state). The extracted pieces:
//  - PreviewGesture.kt  — peek/pin phase machine + gesture Modifier (unit-tested)
//  - PreviewChrome.kt   — PreviewHeader + content-type chip
//  - PreviewContent.kt  — PreviewTextContent/ImageContent/FileContent renderers
//  - PreviewActionRow.kt — pinned-mode action row + relativeTimePreview
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
