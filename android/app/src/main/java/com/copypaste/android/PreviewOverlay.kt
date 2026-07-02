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
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.CpMotion
import com.copypaste.android.ui.theme.CpShapes
import com.copypaste.android.ui.theme.CpSpacing
import com.copypaste.android.ui.theme.CpTypography
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.cpMotionSpec
import com.copypaste.android.ui.theme.rememberCpMotionReduced
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
//
// android-preview S6: re-based on tokens (LocalCpColors/CpTypography/
// CpShapes/CpSpacing/CpMotion) and now owns the `revealed` state (spec.md
// "Preview Reveal (NEW)") threaded down to PreviewTextContent/
// PreviewImageContent/PreviewActionRow, plus the image loading/success/
// failure tri-state (spec.md "Image Preview Loading States").
// ─────────────────────────────────────────────────────────────────────────────

/**
 * spec.md "Image Preview Loading States": distinct loading/success/failure —
 * a nullable bitmap alone cannot distinguish "still decoding" from "decode
 * failed", which previously left a permanent spinner on failure instead of
 * the required explanatory failure state.
 */
sealed class PreviewImageLoadState {
    object Loading : PreviewImageLoadState()
    data class Success(val bitmap: ImageBitmap) : PreviewImageLoadState()
    object Failure : PreviewImageLoadState()
}

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

    val cp = LocalCpColors.current
    val pinned = phase == PreviewPhase.Pinned

    // spec.md "Preview Reveal (NEW)": keyed by item.id, mirroring HistoryRow's
    // `revealed by remember(item.id)` — resets to false whenever a different
    // item is previewed ("Reveal state resets per item").
    var revealed by remember(item.id) { mutableStateOf(false) }

    // Dismiss pinned via system back
    BackHandler(enabled = pinned) { onDismiss() }

    val reducedMotion = rememberCpMotionReduced()

    // Scale-in animation for the card.
    val cardScale by animateFloatAsState(
        targetValue = if (phase == PreviewPhase.Idle) 0.85f else 1f,
        animationSpec = cpMotionSpec(reducedMotion) {
            tween(durationMillis = CpMotion.THEME_MS, easing = CpMotion.Ease)
        },
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

    // Full-res bitmap loaded lazily — decode at ≤1080px to bound memory.
    // spec.md "Image Preview Loading States": Loading/Success/Failure, not a
    // bare nullable bitmap, so a decode failure surfaces an explicit state
    // instead of an indefinite spinner.
    val fullImageState by produceState<PreviewImageLoadState>(
        initialValue = PreviewImageLoadState.Loading,
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
                }.fold(
                    onSuccess = { bmp ->
                        if (bmp != null) PreviewImageLoadState.Success(bmp) else PreviewImageLoadState.Failure
                    },
                    onFailure = { PreviewImageLoadState.Failure },
                )
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
            // The scrim keeps the phase-driven depth cue (pinned is darker than
            // peeking) but now derives its hue from the token ramp (cp.scrim)
            // instead of a raw Color.Black literal.
            .background(cp.scrim.copy(alpha = if (pinned) 0.55f else 0.45f))
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
                .padding(horizontal = CpSpacing.s8, vertical = if (pinned) CpSpacing.s9 else 40.dp)
                .graphicsLayer {
                    scaleX = cardScale
                    scaleY = cardScale
                }
                .background(color = cp.card, shape = RoundedCornerShape(CpShapes.card))
                .clickable(
                    indication = null,
                    interactionSource = remember { MutableInteractionSource() },
                    onClick = {},
                ),
        ) {
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(CpSpacing.s7),
            ) {
                PreviewHeader(
                    item = item,
                    pinned = pinned,
                    onDismiss = if (pinned) onDismiss else null,
                )
                HorizontalDivider(
                    modifier = Modifier.padding(vertical = 10.dp),
                    color = cp.divider,
                    thickness = 0.5.dp,
                )
                Box(modifier = Modifier.weight(1f)) {
                    when {
                        item.isImage -> PreviewImageContent(
                            state = fullImageState,
                            isSensitive = item.isSensitive,
                            maskSensitive = maskSensitive,
                            revealed = revealed,
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
                            revealed = revealed,
                            pinned = pinned,
                        )
                    }
                }
                if (!pinned) {
                    HorizontalDivider(
                        modifier = Modifier.padding(vertical = CpSpacing.s4),
                        color = cp.divider,
                        thickness = 0.5.dp,
                    )
                    Text(
                        text = stringResource(R.string.preview_drag_up_hint),
                        style = CpTypography.micro,
                        color = cp.dim,
                        modifier = Modifier.align(Alignment.CenterHorizontally),
                    )
                } else {
                    HorizontalDivider(
                        modifier = Modifier.padding(vertical = CpSpacing.s4),
                        color = cp.divider,
                        thickness = 0.5.dp,
                    )
                    PreviewActionRow(
                        item = item,
                        revealed = revealed,
                        onReveal = { revealed = true },
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
