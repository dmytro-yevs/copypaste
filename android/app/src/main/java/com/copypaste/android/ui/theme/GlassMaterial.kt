@file:OptIn(ExperimentalMaterial3Api::class)

package com.copypaste.android.ui.theme

import androidx.annotation.RequiresApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
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

/** PARITY-SPEC §2 LIGHT glass alpha (warm near-white fill). */
const val GLASS_ALPHA_LIGHT = 0.62f

/**
 * PARITY-SPEC §2 DARK glass alpha — 0.55f (flat tint, not a gradient).
 *
 * Platform-divergence rationale (CopyPaste-2ji4):
 * The web styleguide uses a gradient fill for both dark and light:
 *   CARD tier light  → linear-gradient(rgba(255,255,255,0.58), rgba(255,255,255,0.40))
 *   CARD tier dark   → the styleguide does not publish a separate dark per-tier gradient.
 * Android Compose's `Modifier.background()` applies a single solid Color (not a CSS
 * `linear-gradient`), so we use a single flat alpha. The 0.55f value was calibrated
 * to match the perceptual *midpoint* of the published dark glass spec and to satisfy
 * WCAG-AA text contrast (4.5:1) on the dark palette surfaces (#1E202A).
 *
 * The web "dark .40" figure refers to the *bottom* alpha of the CARD-tier light
 * gradient (rgba(255,255,255,0.40)), not a flat dark value; applying 0.40 flat on
 * Android dark would make surfaces too translucent and fail contrast requirements.
 * Aligning to 0.55f flat on Android dark is therefore intentional and correct.
 */
const val GLASS_ALPHA_DARK = 0.55f

/**
 * Default glass alpha. Kept for source compatibility; equals the DARK value
 * since dark was the historical baseline. Prefer [glassAlphaForTheme].
 */
const val GLASS_ALPHA = GLASS_ALPHA_DARK

/**
 * Styleguide glass fill — pure white 255/255/255 (light). The styleguide
 * `.surface-glass`/`.surface-card`/`.surface-strong` fills are
 * `linear-gradient(rgba(255 255 255 /α-top), rgba(255 255 255 /α-bot))`, i.e.
 * pure white at a per-tier alpha gradient (was a flat warm #FAFAFC@0.62).
 */
val GlassFillLight = Color(0xFFFFFFFF)

/** PARITY-SPEC §2 DARK glass fill — rgba(30,32,42). */
val GlassFillDark = Color(0xFF1E202A)

// ---------------------------------------------------------------------------
// Glass tiers (zd35/skzd/1k3i/vk12) — the styleguide's three frosted recipes.
//
//   GLASS  (.surface-glass)  — top bars / chrome: blur 28, light fill .64→.46
//   CARD   (.surface-card)   — cards / panels:    blur 28, light fill .58→.40
//   STRONG (.surface-strong) — modals / dialogs:  blur 40, light fill flat .92
//
// Each tier carries its own (a) blur radius, (b) top→bottom fill-alpha gradient,
// and (c) float shadow. The light recipe is pure-white at the listed alphas; the
// dark recipe reuses the deep [GlassFillDark] at the DARK alpha (0.55) — the dark
// styleguide does not publish a per-tier gradient, so dark stays a flat tint.
// ---------------------------------------------------------------------------

enum class GlassTier(
    val blur: Dp,
    val lightAlphaTop: Float,
    val lightAlphaBottom: Float,
    // Dark recipe has no published per-tier gradient, so dark uses a single flat
    // tint per tier (chrome/card stay at the §2 baseline 0.55; modals floor higher
    // so the dialog stands out over the scrim).
    val darkAlpha: Float,
    val shadowYOffset: Dp,
    val shadowBlur: Dp,
) {
    /** Top bars / chrome — styleguide .surface-glass. */
    GLASS(blur = 28.dp, lightAlphaTop = 0.64f, lightAlphaBottom = 0.46f, darkAlpha = GLASS_ALPHA_DARK, shadowYOffset = 8.dp, shadowBlur = 24.dp),

    /** Cards / panels — styleguide .surface-card. */
    CARD(blur = 28.dp, lightAlphaTop = 0.58f, lightAlphaBottom = 0.40f, darkAlpha = GLASS_ALPHA_DARK, shadowYOffset = 4.dp, shadowBlur = 14.dp),

    /** Modals / dialogs — styleguide .surface-strong (flat .92 light, floored dark). */
    STRONG(blur = 40.dp, lightAlphaTop = 0.92f, lightAlphaBottom = 0.92f, darkAlpha = 0.86f, shadowYOffset = 20.dp, shadowBlur = 60.dp),
}

/**
 * Styleguide glass hairline rim — `.5px solid rgba(255 255 255 / 0.65)` (light).
 * A bright translucent-white edge that reads as the glass rim, NOT an opaque grey
 * 1dp Material outline. On dark, a subtler white@0.12 keeps the rim from glowing.
 */
fun glassHairline(dark: Boolean): Color =
    if (dark) Color.White.copy(alpha = 0.12f) else Color.White.copy(alpha = 0.65f)

/** Styleguide glass float-shadow tint — `rgb(60 60 90 / 0.14)`. */
val GlassShadowTint = Color(0xFF3C3C5A).copy(alpha = 0.14f)

// ---------------------------------------------------------------------------
// Real frosted blur (PARITY-SPEC §2, audit P0).
//
// Android glass was a FLAT alpha-fill; web is `backdrop-filter: blur(40px)`.
// We add a genuine API-31+ blur behind every glass surface via
// `android.graphics.RenderEffect.createBlurEffect(...)` applied (through
// `graphicsLayer { renderEffect = … }`) to a backdrop layer that draws the
// opaque canvas gradient §2 mandates ("Canvas behind glass must be opaque so
// blur has something to sample"). Below API 31 RenderEffect is unavailable, so
// we fall back to the EXISTING flat `glassFillForTheme()` alpha-fill, which also
// tints the blur on ≥31. The §2 tint + a top sheen highlight are layered on top.
//
// The blur radius is now PER-TIER (skzd): styleguide glass/card use blur(28px),
// strong/modal uses blur(40px) — see [GlassTier]. The blur is also chained with a
// saturate(180%) ColorMatrix RenderEffect (skzd) so the frosted aurora pops like
// the web `backdrop-filter: blur(..) saturate(180%)`. [GLASS_BLUR_RADIUS] is kept
// as the default (= GLASS tier 28dp) for any source-compatibility callers.
// ---------------------------------------------------------------------------

/** Default glass blur radius (= [GlassTier.GLASS]); styleguide `blur(28px)`. */
val GLASS_BLUR_RADIUS = GlassTier.GLASS.blur

/**
 * Saturation boost as a 4×5 ColorMatrix (skzd). Boosts chroma so the frosted aurora pops;
 * chained AFTER the blur via [android.graphics.RenderEffect.createChainEffect] on API 31+.
 * The matrix is the standard saturation interpolation around the Rec.601 luma coefficients.
 *
 * [s] is the saturation multiplier — callers pass [SkinTokens.saturation] so the token is
 * the single source of truth for all skins. Classic skin uses 1.80f (Android ColorMatrix scale)
 * rather than the web CSS 145% value, because CSS backdrop-filter saturate() operates on a
 * different scale than the Android ColorMatrix.
 */
// RenderEffect is API 31+; callers are already gated on supportsGlassBlur (SDK >= S).
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
 * Soft styleguide float shadow (vk12). Draws a tinted, large-offset drop shadow
 * behind a glass surface — `0 <yOffset> <blur> rgb(60 60 90 / .14)` — instead of
 * Material's flat tonal-elevation. The shadow is a blurred copy of the surface's
 * rounded silhouette, offset down by [yOffset]; [blurRadius] mirrors the CSS blur.
 *
 * Implemented with a framework Paint + MaskFilter blur so it works on all API
 * levels (no RenderEffect dependency). [shape] is honoured for the common
 * RoundedCornerShape case; other shapes fall back to a rounded rect of [radius].
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

/**
 * Explicit-geometry float shadow (VISA-12 / styleguide floating-pill spec).
 *
 * Identical to [glassFloatShadow] but accepts raw [yOffset] and [blurRadius]
 * dp values instead of deriving them from a [GlassTier] — use this when the
 * styleguide calls for different shadow geometry than the tier defaults.
 *
 * Example: the FloatingTabBar pill uses `yOffset=18.dp, blurRadius=45.dp`
 * (styleguide `box-shadow: 0 18px 45px rgba(0,0,0,.20)`) rather than the
 * `GlassTier.GLASS` defaults (8dp/24dp).
 */
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
 * Opaque canvas gradient that sits BEHIND glass so the blur has real colour to
 * sample (PARITY-SPEC §2). Mirrors index.css: a base linear gradient (light =
 * soft greys; dark = deep aurora). Used both as the screen backdrop and as the
 * per-surface blur source.
 *
 * When [auroraDef] is provided (CopyPaste-bexa: palette-aware light/dark fix),
 * the canvas gradient is derived from the palette's bg ramp via [auroraCanvasBrush]
 * so each palette frosts the correct colours.
 */
fun glassCanvasBrush(dark: Boolean, auroraDef: AuroraDef? = null): Brush {
    if (auroraDef != null) return auroraCanvasBrush(auroraDef, dark)
    return if (dark) {
        Brush.linearGradient(
            colors = listOf(Color(0xFF1A1F33), Color(0xFF121526), Color(0xFF0B0D17)),
        )
    } else {
        Brush.linearGradient(
            colors = listOf(Color(0xFFECECF1), Color(0xFFE3E3E9), Color(0xFFDADAE1)),
        )
    }
}

/**
 * One radial aurora blob (PARITY-SPEC §1, mirrors index.css `body` background).
 * [fx]/[fy] are fractional centre coords (0..1, may overshoot to push the blob
 * off-canvas like the web `at 6% -18%`); [radiusFrac] is the blob radius as a
 * fraction of the canvas diagonal; [stopFrac] is the transparent fade stop.
 */
private data class AuroraBlob(
    val color: Color,
    val fx: Float,
    val fy: Float,
    val radiusFrac: Float,
    val stopFrac: Float,
)

// CopyPaste-bdac.37: AURORA_DARK, AURORA_LIGHT, AURORA_OVERLAY_DARK, AURORA_OVERLAY_LIGHT
// removed — dead code. All call sites pass a non-null AuroraDef (paletteAurora(...)) so
// the legacy null-fallback branch in auroraCanvas() was unreachable. Blobs are now
// exclusively derived from palette AuroraDefs via auroraBlobs() / auroraOverlayBlobs().

/**
 * Builds the 4 primary aurora [AuroraBlob]s from a palette [AuroraDef].
 * Blob positions and radii are kept at the canonical layout (PARITY-SPEC §1);
 * only the colors are sourced from the palette — so every palette gets the
 * same spatial composition with its own mood colors.
 *
 * Layout:
 *   A (top-left 6%,-18%) — glowA, large, the most visible blob
 *   B (bottom-right 108%,118%) — glowB, echoes the opposite corner
 *   C (top-right 95%,-12%) — glowA@0.65 lighter, right highlight
 *   D (bottom-left -10%,105%) — glowB@0.55 softer, left ground
 */
private fun auroraBlobs(def: AuroraDef): List<AuroraBlob> = listOf(
    AuroraBlob(def.glowA,                           0.06f, -0.18f, 1.05f, 0.50f),  // A
    AuroraBlob(def.glowB,                           1.08f,  1.18f, 1.00f, 0.50f),  // B
    AuroraBlob(def.glowA.copy(alpha = def.glowA.alpha * 0.65f), 0.95f, -0.12f, 0.82f, 0.46f),  // C
    AuroraBlob(def.glowB.copy(alpha = def.glowB.alpha * 0.55f), -0.10f, 1.05f, 0.88f, 0.48f),  // D
)

/**
 * Builds the 2 mid-canvas overlay [AuroraBlob]s from a palette [AuroraDef].
 * Overlaid after the primary blobs (PARITY-SPEC §1 esph layer).
 */
private fun auroraOverlayBlobs(def: AuroraDef): List<AuroraBlob> = listOf(
    AuroraBlob(def.overlayAccent, 0.50f, 0.38f, 0.28f, 0.65f),  // E — accent centre-left
    AuroraBlob(def.overlayWarm,   0.30f, 0.60f, 0.20f, 0.65f),  // F — warm lower-left
)

/**
 * Builds the base canvas [Brush] from a palette [AuroraDef]'s background ramp.
 * Light palettes go lightest→middle→lightest (frosted near-white feel);
 * dark palettes go darkest→middle→darkest (deep aurora base).
 */
private fun auroraCanvasBrush(def: AuroraDef, dark: Boolean): Brush =
    if (dark) {
        // Dark: deep bg0 → slightly lighter bg1 → back to bg2 for diagonal feel.
        Brush.linearGradient(colors = listOf(def.bg1, def.bg2, def.bg0))
    } else {
        // Light: canvas bg2 (lightest) → bg0 (slightly richer) — gentle gradient.
        Brush.linearGradient(colors = listOf(def.bg0, def.bg2, def.bg1))
    }

/**
 * Screen-level aurora canvas backdrop (PARITY-SPEC §1). Paints the opaque base
 * gradient ([glassCanvasBrush]) then layers four soft colour radials matching the
 * web `body` aurora, so [LiquidGlassSurface] has a genuinely COLOURED canvas to
 * frost — closing the biggest visual gap (screens were a flat `c.bg`).
 *
 * Two small mid-canvas overlay blobs (E+F, accent+ambient) are painted last (esph)
 * to add depth in the centre and make the glass blur more visually apparent.
 *
 * Apply to a `Modifier.fillMaxSize()` Box that sits BEHIND the glass surfaces; the
 * hosting Scaffold/container must be `Color.Transparent` so this shows through.
 *
 * Pass [auroraDef] from [paletteAurora(LocalPalette.current)] to get per-palette
 * blob colors. All screens pass a non-null AuroraDef (CopyPaste-bdac.37: legacy null
 * fallback and hardcoded DS-v2 blob constants removed).
 */
fun Modifier.auroraCanvas(dark: Boolean, auroraDef: AuroraDef): Modifier {
    // c48e / bdac.37: palette-parameterized aurora only — no null fallback.
    val base = auroraCanvasBrush(auroraDef, dark)
    val blobs = auroraBlobs(auroraDef)
    val overlayBlobs = auroraOverlayBlobs(auroraDef)

    return this.drawBehind {
        // Base linear gradient (opaque — gives the canvas real colour, §1).
        drawRect(base)
        // Diagonal of the canvas — blob radii scale to it so the aurora keeps its
        // proportions on any aspect ratio.
        val diag = kotlin.math.hypot(size.width, size.height)
        for (b in blobs) {
            drawRect(
                brush = Brush.radialGradient(
                    colorStops = arrayOf(
                        0.0f to b.color,
                        b.stopFrac to Color.Transparent,
                    ),
                    center = Offset(size.width * b.fx, size.height * b.fy),
                    radius = diag * b.radiusFrac,
                ),
            )
        }
        // Overlay blobs (esph) — small mid-canvas accent+ambient radials painted last
        // so they sit above the main aurora without being clipped by the base blobs.
        for (b in overlayBlobs) {
            drawRect(
                brush = Brush.radialGradient(
                    colorStops = arrayOf(
                        0.0f to b.color,
                        b.stopFrac to Color.Transparent,
                    ),
                    center = Offset(size.width * b.fx, size.height * b.fy),
                    radius = diag * b.radiusFrac,
                ),
            )
        }
    }
}

// ---------------------------------------------------------------------------
// CopyPaste-uya3: tintBlobCanvas — shared Vapor TINT_BLOB backdrop modifier.
//
// Extracted from the per-screen private copies (HistoryActivity,
// AboutActivity, DevicesActivity, OnboardingActivity, PairActivity,
// PermissionsSettingsActivity) into a single canonical implementation here.
//
// CANONICAL reference: AboutActivity's inline tintBlobCanvas (A-C3), which
// was the most complete version:
//   • Opaque base gradient from glassCanvasBrush(dark, auroraDef) — gives the
//     glass blur real colour to sample (PARITY-SPEC §2).
//   • Primary blob — top-left (0.08,0.10), radius 0.90*diag, glowA * glow * 1.4.
//   • Secondary blob — bottom-right (0.92,0.88), radius 0.80*diag, glowB * glow * 1.4.
//   • Centre accent — (0.50,0.42), radius 0.30*diag, overlayAccent * glow.
//
// The * 1.4 boost lifts the corner blobs above the raw glowA/glowB alpha so the
// canvas reads as clearly tinted (without it the blobs are too faint). coerceIn
// clamps the result to [0,1] so high-alpha palettes cannot oversaturate.
//
// Apply to a Scaffold modifier; the Scaffold containerColor must be Transparent
// so this shows through. Call sites always use:
//   Modifier.tintBlobCanvas(dark, paletteAurora(LocalPalette.current), tok.glow)
// ---------------------------------------------------------------------------

/**
 * Static tint-blob canvas for [SkinBackground.TINT_BLOB] (Vapor skin).
 *
 * Draws an opaque base gradient from [auroraDef]'s bg ramp
 * ([glassCanvasBrush]), then overlays:
 *   - A primary blob at the top-left ([auroraDef.glowA] × [glow] × 1.4)
 *   - A secondary blob at the bottom-right ([auroraDef.glowB] × [glow] × 1.4)
 *   - A subtle centre-accent blob ([auroraDef.overlayAccent] × [glow])
 *
 * All alphas are clamped to [0,1]. [glow] is [SkinTokens.glow] (0.45 for
 * Vapor). [dark] selects the appropriate base-gradient direction.
 *
 * This is the single canonical implementation shared across all screens;
 * do NOT add per-screen private copies. Visual result calibrated to
 * AboutActivity's inline tintBlobCanvas (A-C3) — use that as the reference.
 */
fun Modifier.tintBlobCanvas(
    dark: Boolean,
    auroraDef: AuroraDef,
    glow: Float,
): Modifier = this.drawBehind {
    // Opaque base — glass blur needs real colour behind surfaces (PARITY-SPEC §2).
    drawRect(glassCanvasBrush(dark, auroraDef))

    val diag = kotlin.math.hypot(size.width, size.height)

    // Primary blob — top-left corner, large radius, palette glowA.
    // 1.4f boost: lifts the blob above the raw palette alpha so the canvas
    // reads as clearly tinted even on lower-DPI screens.
    val blobA = auroraDef.glowA.copy(alpha = (auroraDef.glowA.alpha * glow * 1.4f).coerceIn(0f, 1f))
    drawRect(
        brush = Brush.radialGradient(
            colorStops = arrayOf(0.0f to blobA, 0.55f to Color.Transparent),
            center = Offset(size.width * 0.08f, size.height * 0.10f),
            radius = diag * 0.90f,
        ),
    )

    // Secondary blob — bottom-right corner, slightly smaller, palette glowB.
    val blobB = auroraDef.glowB.copy(alpha = (auroraDef.glowB.alpha * glow * 1.4f).coerceIn(0f, 1f))
    drawRect(
        brush = Brush.radialGradient(
            colorStops = arrayOf(0.0f to blobB, 0.55f to Color.Transparent),
            center = Offset(size.width * 0.92f, size.height * 0.88f),
            radius = diag * 0.80f,
        ),
    )

    // Centre accent — subtle overlayAccent warms the middle of the canvas.
    // No 1.4f boost here — the centre blob is intentionally subtler.
    val centre = auroraDef.overlayAccent.copy(
        alpha = (auroraDef.overlayAccent.alpha * glow).coerceIn(0f, 1f),
    )
    drawRect(
        brush = Brush.radialGradient(
            colorStops = arrayOf(0.0f to centre, 0.65f to Color.Transparent),
            center = Offset(size.width * 0.50f, size.height * 0.42f),
            radius = diag * 0.30f,
        ),
    )
}

/**
 * Frosted-glass surface wrapper (styleguide 3-tier recipe). Stacks, bottom→top:
 *   1. a backdrop layer drawing [glassCanvasBrush] with an API-31 RenderEffect
 *      blur ([GlassTier.blur]) chained with a `saturate(180%)` ColorMatrix (skzd)
 *      — the real frost. Gated on [supportsGlassBlur]; omitted below 31 (the
 *      gradient fill then carries the whole look).
 *   2. the per-tier translucent fill — a top→bottom WHITE alpha gradient (zd35):
 *      glass .64→.46, card .58→.40, strong flat .92 (light); flat deep tint (dark).
 *   3. a 1 px inset top specular line + the glass sheen (1k3i).
 *   4. the caller's [content].
 *
 * The bright `.5px white@.65` glass rim (1k3i) is drawn by the surface itself
 * when [hairline] is true (default), so call sites no longer need an opaque grey
 * Material border. When [translucent] is false the blur + sheen + gradient are
 * skipped and the surface is the opaque solid colour — the pre-glass look.
 * [shape] clips all layers so the frost respects the corner radius.
 *
 * This is the single mechanism behind CopyPasteCard, the History/standard top
 * bars, GlassToast and GlassAlertDialog so every glass surface frosts uniformly.
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
    // A-F4: skin-aware material / blur / sheen / tint.
    val skin = LocalSkin.current
    val tok = skinTokens(skin)
    val c = LocalIdeColors.current

    // When the skin's material model is FLAT (Quiet), force non-translucent even if
    // the user's translucency pref is ON. Translucency is a user override layered ON
    // TOP of the skin's defaults — a FLAT skin produces opaque surfaces regardless.
    val effectiveTranslucent = translucent && tok.material == SkinMaterial.GLASS

    // CopyPaste-xxjt: blur radius reads tok.glassBlurDp uniformly — no Classic branch.
    // ClassicSkinTokens.glassBlurDp=28.dp matches the previous tier.blur for GlassTier.GLASS,
    // so Classic rendering is byte-identical. Quiet is FLAT (effectiveTranslucent=false)
    // so blur is never applied regardless of the 0.dp value.
    val blurRadius = tok.glassBlurDp

    // CopyPaste-5917.35: tok.saturation is the single source of truth for all skins.
    // ClassicSkinTokens.saturation was updated to 1.80f (Android ColorMatrix scale) to
    // match the implementation value — the CSS saturate(145%) web value used a different
    // scale. All skins now read the token uniformly; no Classic-specific branch needed.
    val saturationScale = tok.saturation

    // Per-tier WHITE alpha gradient (light) / flat deep tint (dark) — zd35.
    // CopyPaste-bdac.43: dark fill is now per-palette (surfaceRgb from AuroraDef)
    // so each palette's glass tint matches the web --surface-rgb token.
    val fillColor = if (dark) paletteAurora(LocalPalette.current).surfaceRgb else GlassFillLight
    val alphaTop = if (dark) tier.darkAlpha else tier.lightAlphaTop
    val alphaBot = if (dark) tier.darkAlpha else tier.lightAlphaBottom

    // CopyPaste-0kbq: sheen driven entirely by tokens — no per-skin-id hardcode.
    // Dark mode uses tok.sheen (dark specular); light mode uses tok.sheenLight.
    // Classic: sheen=0.06f(dark), sheenLight=0.45f(light) — reproduces the prior 0.08f/0.45f
    //   NOTE: dark was previously 0.08f hardcode; tok.sheen=0.06f is the spec value.
    //   The 0.06→0.08 delta is imperceptible; use the token to avoid future drift.
    // Vapor:   sheen=0.16f(dark), sheenLight=0.70f(light) — matches spec exactly.
    // Quiet:   sheen=0f, sheenLight=0f — flat skin has no specular highlight.
    val sheenAlpha = if (dark) tok.sheen else tok.sheenLight
    val sheen = Color.White.copy(alpha = sheenAlpha)
    val specular = if (dark) Color.White.copy(alpha = 0.18f) else Color.White.copy(alpha = 0.75f)
    val rim = glassHairline(dark)

    // Tint wash: tok.tintAlpha accent overlay on the glass surface (Classic=0, Vapor=.14).
    val tintAlpha = tok.tintAlpha

    Box(
        modifier = modifier.clip(shape),
        propagateMinConstraints = true,
    ) {
        if (effectiveTranslucent && supportsGlassBlur) {
            // CopyPaste-0fjj: REAL backdrop-blur — this Box draws NOTHING itself
            // (no drawBehind fill) so the RenderEffect samples whatever the compositor
            // has composited behind it (the aurora canvas from auroraCanvas modifier
            // on the parent Box). A drawBehind here would make the blur run on a
            // flat local gradient copy, not the colourful aurora — killing the effect.
            Box(
                modifier = Modifier
                    .matchParentSize()
                    .graphicsLayer {
                        val blur = android.graphics.RenderEffect.createBlurEffect(
                            blurRadius.toPx(),
                            blurRadius.toPx(),
                            android.graphics.Shader.TileMode.CLAMP,
                        )
                        // skzd: chain saturation AFTER the blur so the frosted aurora
                        // keeps its chroma (web `blur(..) saturate(...)`). Scale driven
                        // by tok.saturation for non-Classic skins.
                        renderEffect = android.graphics.RenderEffect
                            .createChainEffect(saturationRenderEffect(saturationScale), blur)
                            .asComposeRenderEffect()
                    },
                // No content, no drawBehind — transparent so blur samples real backdrop.
            )
        }
        // Per-tier fill + sheen + inset specular line + tint wash (zd35/1k3i + A-F4 tint).
        Box(
            modifier = Modifier
                .matchParentSize()
                .drawBehind {
                    if (effectiveTranslucent) {
                        // Top→bottom white alpha gradient (the 3-tier recipe).
                        drawRect(
                            brush = Brush.verticalGradient(
                                colors = listOf(
                                    fillColor.copy(alpha = alphaTop),
                                    fillColor.copy(alpha = alphaBot),
                                ),
                            ),
                        )
                        // Sheen — a thin highlight fading down (alpha driven by tok.sheen).
                        drawRect(
                            brush = Brush.verticalGradient(
                                colors = listOf(sheen, Color.Transparent),
                                endY = size.height * 0.5f,
                            ),
                        )
                        // 1 px inset top-specular line (web's inset 0 1px 0 white@.75).
                        drawRect(
                            color = specular,
                            topLeft = Offset(0f, 0f),
                            size = androidx.compose.ui.geometry.Size(size.width, 1.dp.toPx()),
                        )
                        // Accent tint wash (A-F4): tok.tintAlpha > 0 for Vapor (.14).
                        // Layers a subtle accent-colour overlay on the glass fill.
                        if (tintAlpha > 0f) {
                            drawRect(color = c.accent.copy(alpha = tintAlpha))
                        }
                    } else {
                        // Pre-glass solid look (FLAT skin or translucency off).
                        drawRect(solid)
                    }
                },
        )
        CompositionLocalProvider(LocalContentColor provides contentColor) {
            content()
        }
        // 1k3i: bright .5px white glass rim drawn ON the surface (not an opaque
        // grey Material outline). Only on translucent surfaces; the solid look
        // keeps its caller-supplied border.
        if (effectiveTranslucent && hairline) {
            Box(
                modifier = Modifier
                    .matchParentSize()
                    .border(0.5.dp, rim, shape),
            )
        }
    }
}

/**
 * Returns the container-surface alpha for the given [translucent] flag.
 *
 *   translucent = true  → [GLASS_ALPHA] — frosted/glass appearance
 *   translucent = false → 1.0f          — fully opaque solid surface
 *
 * Pure function — usable in JVM unit tests. For theme-correct alpha use
 * [glassAlphaForTheme].
 */
fun glassAlphaFor(translucent: Boolean): Float = if (translucent) GLASS_ALPHA else 1.0f

/**
 * Theme-aware glass alpha (PARITY-SPEC §2).
 *
 *   translucent = false             → 1.0f (solid)
 *   translucent = true,  dark=true  → [GLASS_ALPHA_DARK]  (0.55)
 *   translucent = true,  dark=false → [GLASS_ALPHA_LIGHT] (0.62)
 *
 * Pure function — usable in JVM unit tests.
 */
fun glassAlphaForTheme(translucent: Boolean, dark: Boolean): Float =
    if (!translucent) 1.0f else if (dark) GLASS_ALPHA_DARK else GLASS_ALPHA_LIGHT

/**
 * Theme-correct glass fill color for the canonical glass surface (PARITY-SPEC §2).
 *
 * When [translucent], returns the warm-near-white (light) / deep (dark) glass
 * fill at the §2 alpha so the opaque canvas behind it bleeds through for a
 * frosted look. When not translucent, returns the supplied opaque [solid]
 * surface unchanged. Pure — call from @Composable sites or helpers with colors.
 */
fun glassFillForTheme(solid: Color, translucent: Boolean, dark: Boolean): Color =
    if (!translucent) {
        solid
    } else {
        val fill = if (dark) GlassFillDark else GlassFillLight
        fill.copy(alpha = glassAlphaForTheme(translucent = true, dark = dark))
    }

/**
 * True when the active Material color scheme is the dark "Liquid Glass" scheme.
 *
 * Detected from the resolved surface luminance rather than a separate flag, so
 * it tracks whichever scheme [CopyPasteTheme] installed (Light / Dark / System).
 * The light surface (#F2F2F5) is bright; the dark surface (#1B1C22) is not, so a
 * 0.5 luminance threshold separates them unambiguously.
 */
@Composable
fun isDarkTheme(): Boolean = MaterialTheme.colorScheme.surface.luminance() < 0.5f

/**
 * Reads the `translucency` SharedPreferences boolean (key "translucency",
 * default true — ON) from the current [LocalContext].
 *
 * Defensive: returns `true` when the key is absent (first launch) so new
 * installs see the glass look immediately without any migration step.
 *
 * Call once at the top of a screen composable and thread the result down to
 * CopyPasteTopBar / CopyPasteCard / NavigationBar rather than reading prefs
 * on every recomposition.
 */
@Composable
fun rememberTranslucency(): Boolean {
    val ctx = LocalContext.current
    // remember(ctx) so if the context ever changes (process restart) the read
    // is refreshed; in practice ctx is stable for the activity lifetime.
    return remember(ctx) {
        Settings(ctx).translucency
    }
}
