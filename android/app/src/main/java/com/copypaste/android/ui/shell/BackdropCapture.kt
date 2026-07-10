package com.copypaste.android.ui.shell

import android.graphics.Picture
import android.graphics.RenderEffect
import android.graphics.Shader
import android.os.Build
import androidx.annotation.RequiresApi
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.Snapshot
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
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive

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
 *
 * [generation] and [tick] are two deliberately ONE-DIRECTIONAL channels — each
 * is written by exactly one side and read by the other, never both in the
 * same scope:
 *  - [generation]: source-writes (inside [Modifier.captureBackdrop]'s own
 *    `drawWithContent`), consumer-reads (inside [CapturedBackdropBlur]'s
 *    `drawWithContent`) — "the source just recorded a new frame".
 *  - [tick]: consumer-writes (the bounded throttled refresh loop in
 *    [CapturedBackdropBlur]), source-reads (inside [Modifier.captureBackdrop]'s
 *    `drawWithContent`) — "re-run the capture even though nothing in the
 *    source subtree itself changed" (e.g. a scrolled child repainted only its
 *    own hardware layer, which does not by itself invalidate the parent's
 *    draw phase). Mixing read+write of the SAME field in one scope is exactly
 *    what caused the self-invalidation bug fixed below (CopyPaste-6jk9) — see
 *    the `Snapshot.withoutReadObservation` note on [generation]'s writer.
 */
class BackdropCaptureState {
    internal val picture = Picture()

    var originInRoot: Offset by mutableStateOf(Offset.Zero)
        internal set

    var generation: Int by mutableIntStateOf(0)
        internal set

    /** See the two-channel note above — consumer-written, source-read only. */
    var tick: Int by mutableIntStateOf(0)
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
            // Deliberate observed read: the only trigger that re-runs this draw
            // phase when a child repaints its own hardware layer without
            // otherwise invalidating this parent (e.g. a scrolled LazyColumn
            // item) — see the two-channel note on BackdropCaptureState.
            state.tick
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
            // This write must NOT be read-observed by this drawWithContent
            // scope: an observed get-then-set on the same snapshot state
            // inside the scope that reads it self-invalidates every frame,
            // causing the source to redraw continuously even with zero
            // consumers (confirmed by on-device isolation).
            Snapshot.withoutReadObservation { state.generation++ }
        }
}

/**
 * Ceiling on how often [CapturedBackdropBlur]'s bounded refresh loop bumps
 * [BackdropCaptureState.tick] (CopyPaste-9u7l) — a scroll-freshness floor, not
 * a frame-rate target: 100ms keeps the pill's backdrop visibly current during
 * a scroll while capping the extra draw work to ~10 forced re-captures/sec
 * for as long as this composable is on screen.
 */
private const val BackdropRefreshIntervalMillis = 100L

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
    // Bounded, throttled freshness refresh (CopyPaste-9u7l): the capture
    // source's own draw phase does not always re-run when only an unrelated
    // descendant's hardware layer changes (e.g. LazyColumn scroll), so the
    // backdrop can go stale. This loop bumps `state.tick` at most once per
    // BackdropRefreshIntervalMillis while this composable is part of the
    // composition — its lifetime is scoped to THIS composable (cancelled
    // automatically when it leaves composition), and it is only composed at
    // all when REAL_BACKDROP blur + the pill are both visible (NavPill.kt's
    // existing `realBackdrop` gate), so the battery cost is bounded to
    // "pill on screen with real blur enabled", not global.
    //
    // Uses `delay(...)`, not `withFrameNanos`: an awaiter registered on the
    // main choreographer clock is treated by Compose UI test's idling
    // resource as a pending recomposition forever, so
    // `ComposeTestRule.waitForIdle()` times out with
    // "possibly due to compose being busy" as soon as this loop is composed
    // (confirmed empirically running BackdropScrollFreshnessConnectedTest —
    // see its own kdoc). `delay` suspends off the frame clock, so Espresso's
    // idling check does not see it as perpetually busy.
    LaunchedEffect(state) {
        while (isActive) {
            delay(BackdropRefreshIntervalMillis)
            state.tick++
        }
    }
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
