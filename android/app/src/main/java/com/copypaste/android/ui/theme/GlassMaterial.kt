@file:OptIn(ExperimentalMaterial3Api::class)

package com.copypaste.android.ui.theme

import androidx.annotation.RequiresApi
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.asComposeRenderEffect
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.copypaste.android.Settings

// ---------------------------------------------------------------------------
// Frosted-glass material (translucency axis only).
//
// The old Liquid-Glass multi-skin/palette system is removed (STYLEGUIDE §6, §11).
// What remains is a single optional "Translucency" treatment: when on, surfaces
// frost the content behind them (real RenderEffect blur ≥ API 31, flat tint
// fallback below). When off, surfaces are the opaque theme colour. No animated
// blobs, no per-skin tokens — only the two-axis theme × accent system.
// ---------------------------------------------------------------------------

/** LIGHT glass alpha (warm near-white fill). */
const val GLASS_ALPHA_LIGHT = 0.62f

/** DARK glass alpha — flat tint (not a gradient). */
const val GLASS_ALPHA_DARK = 0.55f

/** Default glass alpha (equals the DARK value). Prefer [glassAlphaForTheme]. */
const val GLASS_ALPHA = GLASS_ALPHA_DARK

/** Glass fill — pure white (light). */
val GlassFillLight = Color(0xFFFFFFFF)

/** Glass fill — deep tint (dark). */
val GlassFillDark = Color(0xFF1E202A)

// ---------------------------------------------------------------------------
// Glass tiers — three frosted recipes (chrome / card / modal).
// Each tier carries its blur radius, a top→bottom light fill-alpha gradient,
// and a float-shadow geometry. Dark uses a single flat tint per tier.
// ---------------------------------------------------------------------------

enum class GlassTier(
    val blur: Dp,
    val lightAlphaTop: Float,
    val lightAlphaBottom: Float,
    val darkAlpha: Float,
    val shadowYOffset: Dp,
    val shadowBlur: Dp,
) {
    /** Top bars / chrome. */
    GLASS(blur = 28.dp, lightAlphaTop = 0.64f, lightAlphaBottom = 0.46f, darkAlpha = GLASS_ALPHA_DARK, shadowYOffset = 8.dp, shadowBlur = 24.dp),

    /** Cards / panels. */
    CARD(blur = 28.dp, lightAlphaTop = 0.58f, lightAlphaBottom = 0.40f, darkAlpha = GLASS_ALPHA_DARK, shadowYOffset = 4.dp, shadowBlur = 14.dp),

    /** Modals / dialogs (flat .92 light, floored dark). */
    STRONG(blur = 40.dp, lightAlphaTop = 0.92f, lightAlphaBottom = 0.92f, darkAlpha = 0.86f, shadowYOffset = 20.dp, shadowBlur = 60.dp),
}

/** Glass hairline rim — bright translucent white edge (light) / subtler (dark). */
fun glassHairline(dark: Boolean): Color =
    if (dark) Color.White.copy(alpha = 0.12f) else Color.White.copy(alpha = 0.65f)

/** Glass float-shadow tint — `rgb(60 60 90 / 0.14)`. */
val GlassShadowTint = Color(0xFF3C3C5A).copy(alpha = 0.14f)

/** Default glass blur radius (= [GlassTier.GLASS]). */
val GLASS_BLUR_RADIUS = GlassTier.GLASS.blur

/**
 * Saturation boost as a 4×5 ColorMatrix — chained AFTER the blur on API 31+ so
 * the frosted backdrop keeps its chroma (web `backdrop-filter: blur(..) saturate(..)`).
 */
@RequiresApi(android.os.Build.VERSION_CODES.S)
private fun saturationRenderEffect(s: Float = 1.8f): android.graphics.RenderEffect {
    val lumaR = 0.213f
    val lumaG = 0.715f
    val lumaB = 0.072f
    val sr = (1f - s) * lumaR
    val sg = (1f - s) * lumaG
    val sb = (1f - s) * lumaB
    val m = floatArrayOf(
        sr + s, sg,     sb,     0f, 0f,
        sr,     sg + s, sb,     0f, 0f,
        sr,     sg,     sb + s, 0f, 0f,
        0f,     0f,     0f,     1f, 0f,
    )
    return android.graphics.RenderEffect.createColorFilterEffect(
        android.graphics.ColorMatrixColorFilter(android.graphics.ColorMatrix(m)),
    )
}

/**
 * Soft float shadow (STYLEGUIDE §5 elevation). Draws a tinted, large-offset drop
 * shadow behind a glass surface instead of Material's flat tonal-elevation.
 */
fun Modifier.glassFloatShadow(
    tier: GlassTier,
    radius: Dp,
    tint: Color = GlassShadowTint,
): Modifier = this.drawBehind {
    val yPx = tier.shadowYOffset.toPx()
    val blurPx = tier.shadowBlur.toPx()
    if (blurPx <= 0f) return@drawBehind
    val rPx = radius.toPx()
    val paint = android.graphics.Paint().apply {
        isAntiAlias = true
        color = tint.toArgb()
        maskFilter = android.graphics.BlurMaskFilter(blurPx, android.graphics.BlurMaskFilter.Blur.NORMAL)
    }
    drawIntoCanvas { canvas ->
        canvas.nativeCanvas.drawRoundRect(
            0f, yPx, size.width, size.height + yPx, rPx, rPx, paint,
        )
    }
}

/** Explicit-geometry float shadow (used by the floating-pill tab bar, §9.12). */
fun Modifier.glassFloatShadowExplicit(
    yOffset: Dp,
    blurRadius: Dp,
    radius: Dp,
    tint: Color = GlassShadowTint,
): Modifier = this.drawBehind {
    val yPx = yOffset.toPx()
    val blurPx = blurRadius.toPx()
    if (blurPx <= 0f) return@drawBehind
    val rPx = radius.toPx()
    val paint = android.graphics.Paint().apply {
        isAntiAlias = true
        color = tint.toArgb()
        maskFilter = android.graphics.BlurMaskFilter(blurPx, android.graphics.BlurMaskFilter.Blur.NORMAL)
    }
    drawIntoCanvas { canvas ->
        canvas.nativeCanvas.drawRoundRect(
            0f, yPx, size.width, size.height + yPx, rPx, rPx, paint,
        )
    }
}

/** True when the platform can render a real RenderEffect blur (API 31+). */
val supportsGlassBlur: Boolean
    get() = android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.S

/**
 * Opaque canvas gradient behind glass so the blur has real colour to sample.
 * A calm neutral gradient — light greys / deep neutrals (STYLEGUIDE §6).
 */
fun glassCanvasBrush(dark: Boolean): Brush =
    if (dark) {
        Brush.linearGradient(colors = listOf(Color(0xFF15161D), Color(0xFF101118), Color(0xFF0B0C12)))
    } else {
        Brush.linearGradient(colors = listOf(Color(0xFFECECF1), Color(0xFFE3E3E9), Color(0xFFDADAE1)))
    }

/**
 * Opaque screen backdrop for translucent screens — paints [glassCanvasBrush] so
 * frosted surfaces have real colour to sample (STYLEGUIDE §6). Apply to a
 * `fillMaxSize` Scaffold modifier
 * whose container colour is Transparent.
 */
fun Modifier.screenCanvas(dark: Boolean): Modifier =
    this.drawBehind { drawRect(glassCanvasBrush(dark)) }

/**
 * Frosted-glass surface wrapper. Stacks, bottom→top:
 *   1. (translucent, ≥ API 31) a backdrop layer with a real RenderEffect blur
 *      ([GlassTier.blur]) chained with a `saturate(180%)` ColorMatrix.
 *   2. the per-tier translucent fill (white alpha gradient light / flat tint dark).
 *   3. a 1px inset top specular line + sheen.
 *   4. the caller's [content].
 *
 * When [translucent] is false the surface is the opaque [solid] colour.
 */
@Composable
fun LiquidGlassSurface(
    shape: Shape,
    translucent: Boolean,
    dark: Boolean,
    solid: Color,
    modifier: Modifier = Modifier,
    tier: GlassTier = GlassTier.CARD,
    contentColor: Color = LocalIdeColors.current.text,
    hairline: Boolean = true,
    content: @Composable BoxScope.() -> Unit,
) {
    val fillColor = if (dark) GlassFillDark else GlassFillLight
    val alphaTop = if (dark) tier.darkAlpha else tier.lightAlphaTop
    val alphaBot = if (dark) tier.darkAlpha else tier.lightAlphaBottom

    val sheenAlpha = if (dark) 0.06f else 0.45f
    val sheen = Color.White.copy(alpha = sheenAlpha)
    val specular = if (dark) Color.White.copy(alpha = 0.18f) else Color.White.copy(alpha = 0.75f)
    val rim = glassHairline(dark)

    Box(
        modifier = modifier.clip(shape),
        propagateMinConstraints = true,
    ) {
        if (translucent && supportsGlassBlur) {
            // REAL backdrop-blur — this Box draws nothing so the RenderEffect
            // samples whatever the compositor has behind it.
            Box(
                modifier = Modifier
                    .matchParentSize()
                    .graphicsLayer {
                        val blur = android.graphics.RenderEffect.createBlurEffect(
                            tier.blur.toPx(),
                            tier.blur.toPx(),
                            android.graphics.Shader.TileMode.CLAMP,
                        )
                        renderEffect = android.graphics.RenderEffect
                            .createChainEffect(saturationRenderEffect(1.8f), blur)
                            .asComposeRenderEffect()
                    },
            )
        }
        Box(
            modifier = Modifier
                .matchParentSize()
                .drawBehind {
                    if (translucent) {
                        drawRect(
                            brush = Brush.verticalGradient(
                                colors = listOf(
                                    fillColor.copy(alpha = alphaTop),
                                    fillColor.copy(alpha = alphaBot),
                                ),
                            ),
                        )
                        drawRect(
                            brush = Brush.verticalGradient(
                                colors = listOf(sheen, Color.Transparent),
                                endY = size.height * 0.5f,
                            ),
                        )
                        drawRect(
                            color = specular,
                            topLeft = Offset(0f, 0f),
                            size = androidx.compose.ui.geometry.Size(size.width, 1.dp.toPx()),
                        )
                    } else {
                        drawRect(solid)
                    }
                },
        )
        CompositionLocalProvider(LocalContentColor provides contentColor) {
            content()
        }
        if (translucent && hairline) {
            Box(
                modifier = Modifier
                    .matchParentSize()
                    .border(0.5.dp, rim, shape),
            )
        }
    }
}

/** Container-surface alpha for the given [translucent] flag (pure; JVM-testable). */
fun glassAlphaFor(translucent: Boolean): Float = if (translucent) GLASS_ALPHA else 1.0f

/** Theme-aware glass alpha (pure; JVM-testable). */
fun glassAlphaForTheme(translucent: Boolean, dark: Boolean): Float =
    if (!translucent) 1.0f else if (dark) GLASS_ALPHA_DARK else GLASS_ALPHA_LIGHT

/** Theme-correct glass fill colour (pure). */
fun glassFillForTheme(solid: Color, translucent: Boolean, dark: Boolean): Color =
    if (!translucent) {
        solid
    } else {
        val fill = if (dark) GlassFillDark else GlassFillLight
        fill.copy(alpha = glassAlphaForTheme(translucent = true, dark = dark))
    }

/** True when the active Material color scheme is the dark scheme. */
@Composable
fun isDarkTheme(): Boolean = MaterialTheme.colorScheme.surface.luminance() < 0.5f

/**
 * Reads the `translucency` SharedPreferences boolean (key "translucency",
 * default true). Optional boolean that may remain in Settings (STYLEGUIDE §2).
 */
@Composable
fun rememberTranslucency(): Boolean {
    val ctx = LocalContext.current
    return remember(ctx) { Settings(ctx).translucency }
}
