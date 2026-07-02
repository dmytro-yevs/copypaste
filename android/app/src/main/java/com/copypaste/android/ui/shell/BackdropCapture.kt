package com.copypaste.android.ui.shell

import android.graphics.Picture
import android.graphics.RenderEffect
import android.graphics.Shader
import android.os.Build
import androidx.annotation.RequiresApi
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Canvas
import androidx.compose.ui.graphics.asComposeRenderEffect
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInRoot
import androidx.compose.ui.unit.Dp

// ---------------------------------------------------------------------------
// BackdropCapture — the real Haze-style "captured-layer" backdrop-blur source
// (design.md D7, android-navigation-chrome "Backdrop blur samples content
// behind the pill"). [Modifier.captureBackdrop] RECORDS the draw commands of
// whatever it is applied to into a shared, reusable `android.graphics.Picture`
// (public API since 1 — a display list of draw commands, the same "record
// once, replay many times" mechanism `View.draw(Picture)`/`WebView` have used
// for years), then immediately replays that SAME recording onto the real
// canvas so the source still renders completely normally.
//
// A consumer positioned anywhere else in the same window (e.g. [NavPill] via
// [CapturedBackdropBlur]) draws a translated COPY of that [Picture] inside a
// `Modifier.graphicsLayer { renderEffect = ... }` layer, so Compose's own
// (already-proven, GPU- AND software-renderer-compatible) layer/RenderEffect
// machinery does the actual blurring — never a hand-rolled
// `Canvas.drawRenderNode`, which throws `IllegalArgumentException("Software
// rendering doesn't support drawRenderNode")` under Paparazzi's LayoutLib
// software canvas (confirmed empirically while building this fix: `RenderNode`
// capture-and-replay is real-device-only, so it cannot be the mechanism a
// golden-tested composable uses). `Picture` has no such restriction — it
// works identically under software (Paparazzi) and hardware-accelerated
// (real device) canvases, which is why it is the capture primitive here
// instead of `RenderNode`.
//
// The source subtree's draw commands are captured exactly ONCE per frame at
// the draw phase — nothing is recomposed or drawn a second time at the
// Compose level, so no LaunchedEffect/ViewModel side effect anywhere in the
// captured subtree fires twice. This replaces the S4 draft's
// double-composition duplicate-and-offset spike technique
// (`BlurSpikeActivity.kt`), which is deliberately spike-only per its own
// kdoc.
// ---------------------------------------------------------------------------

/**
 * Shared capture state for one backdrop source. [originInRoot] is the
 * source's current top-left in root (window) coordinates, published by
 * [Modifier.captureBackdrop] so a consumer elsewhere in the tree can compute
 * the translation needed to align a blurred copy with its own on-screen
 * position. [generation] is bumped on every recording — a consumer reads it
 * inside its own draw phase to subscribe to Compose's snapshot system, so it
 * redraws whenever the source redraws (e.g. on scroll) with no explicit
 * invalidation wiring between the two composables.
 */
class BackdropCaptureState {
    internal val picture = Picture()

    var originInRoot: Offset by mutableStateOf(Offset.Zero)
        internal set

    var generation: Int by mutableIntStateOf(0)
        internal set
}

/**
 * Marks the modified content as a backdrop-capture SOURCE: it draws exactly
 * as it would without this modifier, and additionally records the same draw
 * commands into [state]'s shared [Picture] for [CapturedBackdropBlur] to
 * sample. A null [state] (blur disabled, or API<31 — real blur itself stays
 * 31+ gated even though `Picture` has no such requirement) makes this a
 * no-op — callers apply it unconditionally.
 */
fun Modifier.captureBackdrop(state: BackdropCaptureState?): Modifier {
    if (state == null) return this
    return this
        .onGloballyPositioned { coordinates -> state.originInRoot = coordinates.positionInRoot() }
        .drawWithContent {
            val w = size.width.toInt()
            val h = size.height.toInt()
            if (w <= 0 || h <= 0) {
                drawContent()
                return@drawWithContent
            }
            val recordingCanvas = state.picture.beginRecording(w, h)
            val realCanvas = drawContext.canvas
            // Redirect this draw pass into the Picture's own recording canvas
            // so `drawContent()` records draw commands rather than painting
            // the visible frame directly.
            drawContext.canvas = Canvas(recordingCanvas)
            drawContent()
            drawContext.canvas = realCanvas
            state.picture.endRecording()
            // Replay the just-recorded frame onto the real (visible) canvas —
            // the Picture capture is a side channel, not a replacement for
            // this content's own on-screen drawing.
            realCanvas.nativeCanvas.drawPicture(state.picture)
            state.generation++
        }
}

/**
 * Draws a blurred, translated copy of [state]'s captured backdrop, clipped to
 * this composable's own bounds — the real "backdrop blur" half of D7 (never
 * `Modifier.blur` on the consumer's own layer). Renders nothing until
 * [state]'s source has published a first frame; the caller is expected to
 * size/position this composable to its own on-screen bounds (e.g.
 * `Modifier.matchParentSize()` inside an already-clipped parent).
 */
@RequiresApi(Build.VERSION_CODES.S)
@Composable
fun CapturedBackdropBlur(
    state: BackdropCaptureState,
    blurRadius: Dp,
    modifier: Modifier = Modifier,
) {
    var originInRoot by remember { mutableStateOf(Offset.Zero) }
    Box(
        modifier = modifier
            .onGloballyPositioned { originInRoot = it.positionInRoot() }
            .graphicsLayer {
                renderEffect = RenderEffect
                    .createBlurEffect(blurRadius.toPx(), blurRadius.toPx(), Shader.TileMode.CLAMP)
                    .asComposeRenderEffect()
            }
            .drawWithContent {
                // Read `generation` so this draw phase re-subscribes whenever
                // the source records a new frame (BackdropCaptureState kdoc).
                state.generation
                val translate = state.originInRoot - originInRoot
                val nativeCanvas = drawContext.canvas.nativeCanvas
                nativeCanvas.save()
                nativeCanvas.translate(translate.x, translate.y)
                nativeCanvas.drawPicture(state.picture)
                nativeCanvas.restore()
            },
    )
}
