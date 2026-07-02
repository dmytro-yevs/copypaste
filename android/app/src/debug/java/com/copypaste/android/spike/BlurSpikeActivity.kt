package com.copypaste.android.spike

import android.graphics.RenderEffect
import android.graphics.Shader
import android.os.Build
import android.os.Bundle
import android.view.Choreographer
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.wrapContentSize
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asComposeRenderEffect
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.boundsInParent
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.roundToInt

/**
 * S0.5 spike (B2, CopyPaste-myh8): prototype of the backdrop-blur
 * "captured-layer strategy" for the translucency appearance axis.
 *
 * Android has no CSS-style `backdrop-filter`. Compose UI on this workspace's
 * pinned BOM (2024.04.01 -> Compose UI ~1.6.x) predates the stable
 * `androidx.compose.ui.graphics.layer.GraphicsLayer` capture API (stable only
 * from Compose UI 1.7), so a graphicsLayer{ renderEffect = ... } box with NO
 * drawn content of its own does not reliably sample sibling content already
 * composited to the surface beneath it -- RenderEffect blurs only the
 * receiving RenderNode's own rendered output.
 *
 * The technique below instead explicitly RE-DRAWS ("captures") a duplicate of
 * the backdrop content into an isolated graphicsLayer, offset to line up
 * pixel-for-pixel with the real backdrop (including live scroll position),
 * then blurs that duplicate layer and clips it to the translucent panel's
 * bounds. This is the same duplicate-then-blur shape used by the deleted
 * pre-2-axis GlassMaterial.kt (see `git log --all -- '*GlassMaterial*'`) and
 * by third-party Compose blur libraries that predate GraphicsLayer capture.
 * It is deliberately spike-quality: double draw cost, no caching, no
 * animation-aware invalidation -- NOT meant to be lifted verbatim into S1.
 */
class BlurSpikeActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            MaterialTheme {
                Surface(modifier = Modifier.fillMaxSize()) {
                    BlurSpikeScreen()
                }
            }
        }
    }
}

private val PANEL_BLUR_RADIUS = 48.dp
private val PANEL_HEIGHT = 260.dp
private const val PANEL_WIDTH_FRACTION = 0.86f
private const val BACKDROP_ROW_COUNT = 40
private val BACKDROP_ROW_HEIGHT = 96.dp

@Composable
private fun BlurSpikeScreen() {
    val scrollState = rememberScrollState()
    var panelBounds by remember { mutableStateOf(Rect.Zero) }

    BoxWithConstraints(modifier = Modifier.fillMaxSize()) {
        val fullWidth = maxWidth

        // Layer 0 -- the real, interactive scrollable backdrop the user scrolls.
        ColorfulBackdrop(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(scrollState),
        )

        val panelModifier = Modifier
            .align(Alignment.Center)
            .fillMaxWidth(PANEL_WIDTH_FRACTION)
            .height(PANEL_HEIGHT)
            .onGloballyPositioned { coords -> panelBounds = coords.boundsInParent() }
            .clip(RoundedCornerShape(28.dp))

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            // Layer 1 -- captured-layer blur: a duplicate backdrop draw, offset
            // to align with layer 0 (panel position + live scroll), confined
            // to the panel's clipped bounds, then blurred via RenderEffect.
            Box(
                modifier = panelModifier.graphicsLayer {
                    renderEffect = RenderEffect
                        .createBlurEffect(
                            PANEL_BLUR_RADIUS.toPx(),
                            PANEL_BLUR_RADIUS.toPx(),
                            Shader.TileMode.CLAMP,
                        )
                        .asComposeRenderEffect()
                },
            ) {
                val offsetX = -panelBounds.left.roundToInt()
                val offsetY = -(panelBounds.top.roundToInt() - scrollState.value)
                Box(
                    modifier = Modifier
                        .wrapContentSize(unbounded = true)
                        .graphicsLayer {
                            translationX = offsetX.toFloat()
                            translationY = offsetY.toFloat()
                        },
                ) {
                    ColorfulBackdrop(modifier = Modifier.width(fullWidth))
                }
            }
        } else {
            // Fallback trigger: Build.VERSION.SDK_INT < 31 has no RenderEffect
            // blur -- static opaque-ish scrim only (per design.md "Blur-disable
            // signal": translucency-off OR API<31; no partial/animated fallback).
            Box(modifier = panelModifier.background(Color.Black.copy(alpha = 0.55f)))
        }

        // Translucent tint + hairline + label, drawn above the blur layer.
        Box(
            modifier = panelModifier
                .background(Color.White.copy(alpha = 0.16f))
                .border(1.dp, Color.White.copy(alpha = 0.35f), RoundedCornerShape(28.dp)),
        ) {
            Text(
                text = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                    "Translucent panel (real blur, API ${Build.VERSION.SDK_INT})"
                } else {
                    "Translucent panel (scrim fallback, API ${Build.VERSION.SDK_INT} < 31)"
                },
                color = Color.White,
                modifier = Modifier
                    .align(Alignment.Center)
                    .padding(16.dp),
            )
        }

        FrameTimeOverlay(modifier = Modifier.align(Alignment.TopEnd).padding(16.dp))
    }
}

/** Scrollable content behind the translucent panel -- gives the blur real colour to sample. */
@Composable
private fun ColorfulBackdrop(modifier: Modifier = Modifier) {
    Column(modifier = modifier) {
        repeat(BACKDROP_ROW_COUNT) { index ->
            val hue = (index * 360f / BACKDROP_ROW_COUNT) % 360f
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(BACKDROP_ROW_HEIGHT)
                    .background(Color.hsv(hue, 0.65f, 0.85f)),
                contentAlignment = Alignment.CenterStart,
            ) {
                Text(text = "Row $index", color = Color.Black, modifier = Modifier.padding(16.dp))
            }
        }
    }
}

/**
 * Choreographer-driven frame-time overlay -- surfaces jank while scrolling
 * behind the blurred panel so a manual on-device run can eyeball dropped
 * frames (this is a debug aid; it is not itself a measurement artifact).
 */
@Composable
private fun FrameTimeOverlay(modifier: Modifier = Modifier) {
    var lastFrameMs by remember { mutableStateOf(0.0) }
    var avgFrameMs by remember { mutableStateOf(0.0) }

    DisposableEffect(Unit) {
        var previousFrameNanos = 0L
        val recentFramesMs = ArrayDeque<Double>()
        val choreographer = Choreographer.getInstance()
        var callback: Choreographer.FrameCallback? = null
        callback = Choreographer.FrameCallback { frameTimeNanos ->
            if (previousFrameNanos != 0L) {
                val deltaMs = (frameTimeNanos - previousFrameNanos) / 1_000_000.0
                lastFrameMs = deltaMs
                recentFramesMs.addLast(deltaMs)
                if (recentFramesMs.size > 60) recentFramesMs.removeFirst()
                avgFrameMs = recentFramesMs.average()
            }
            previousFrameNanos = frameTimeNanos
            choreographer.postFrameCallback(callback!!)
        }
        choreographer.postFrameCallback(callback)
        onDispose { choreographer.removeFrameCallback(callback) }
    }

    Box(
        modifier = modifier
            .clip(RoundedCornerShape(8.dp))
            .background(Color.Black.copy(alpha = 0.6f))
            .padding(8.dp),
    ) {
        Column {
            Text(
                text = "frame: %.1f ms".format(lastFrameMs),
                color = Color.White,
                fontSize = 12.sp,
            )
            Text(
                text = "avg(60): %.1f ms".format(avgFrameMs),
                color = Color.White,
                fontSize = 12.sp,
            )
        }
    }
}
