package com.copypaste.android.ui.theme

import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color

// ---------------------------------------------------------------------------
// Palette system — switchable design token ramps (c48e Liquid-Glass refresh)
//
// Source of truth: /tmp/sg/source.html palettes §2516–2597 + contrastProfiles
// §2599–2628 (balanced dark / balanced light contrast profiles).
//
// Each Palette maps to:
//   • an IdeColors ramp     (27 semantic tokens; LocalIdeColors)
//   • a LiquidTokens holder (accent2/accent3/glass params/glow/motionScale)
//   • an AuroraDef          (blob colors for auroraCanvas parameterization)
//   • a Material3 ColorScheme produced in Theme.kt
//
// The active palette is stored in SharedPreferences key "palette" (default
// GRAPHITE_MIST). CopyPasteTheme reads it and provides all three locals.
// Future palette picker: read LocalPalette.current, write Settings.palette.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Palette enum
// ---------------------------------------------------------------------------

/**
 * Switchable visual palette. Each entry drives the full IdeColors ramp,
 * LiquidTokens, AuroraDef, and Material3 ColorScheme.
 *
 * Scheme "dark" palettes use the balanced-dark contrast profile (text @.96,
 * dim @.78, faint @.58, mute @.46, icon @.92, line white@.14, border @.16).
 * Scheme "light" palettes use the balanced-light contrast profile
 * (text rgba(13,20,32,.92), dim rgba(35,47,67,.76), etc.).
 */
enum class Palette(
    /** Whether this palette renders as a dark scheme. */
    val isDark: Boolean,
) {
    // ── Dark palettes ──────────────────────────────────────────────────────
    /** Graphite Mist — cool grey/blue, the new DEFAULT dark palette (c48e). */
    GRAPHITE_MIST(isDark = true),
    /** Liquid Blue — deep blue/violet, the existing Design-System-v2 dark ramp. */
    LIQUID_BLUE(isDark = true),
    /** Deep Sky — vivid electric-blue dark ramp. */
    DEEP_SKY(isDark = true),
    /** Nordic Cyan — dark teal/cyan ramp. */
    NORDIC_CYAN(isDark = true),
    /** Aurora Violet — deep purple/pink dark ramp. */
    AURORA_VIOLET(isDark = true),
    /** Amber Night — warm amber+blue dark ramp. */
    AMBER_NIGHT(isDark = true),
    // ── Light palettes ─────────────────────────────────────────────────────
    /** Cloud Silver — cool light grey (Apple-adjacent). */
    CLOUD_SILVER(isDark = false),
    /** Frost Blue — light icy blue. */
    FROST_BLUE(isDark = false),
    /** Porcelain — grey-blue light. */
    PORCELAIN(isDark = false),
    /** Pearl Grey — neutral grey light. */
    PEARL_GREY(isDark = false),
    ;

    companion object {
        /** The default palette — Graphite Mist dark (c48e). */
        val DEFAULT = GRAPHITE_MIST
    }
}

// ---------------------------------------------------------------------------
// LiquidTokens — extended glass/motion tokens not covered by IdeColors
// ---------------------------------------------------------------------------

/**
 * Extended liquid-glass tokens for each palette. Carried alongside IdeColors
 * via [LocalLiquidTokens]. Screens that need glass-opacity or aurora glow
 * multipliers read these rather than hardcoding values.
 *
 * [glassOpacity]  — base opacity for glass fill layers (dark styleguide .64)
 * [glassBlurDp]   — blur radius in dp for LiquidGlassSurface (styleguide 28)
 * [saturation]    — backdrop-filter saturation multiple (1.45 for Graphite Mist;
 *                   standard is 1.8 per existing saturationRenderEffect; each
 *                   palette can tune this subtly — stored as reference, not
 *                   yet fed into RenderEffect which stays at 1.8 for parity)
 * [glow]          — aurora glow multiplier (0..1) — scales blob alphas
 * [motionScale]   — cinematic (1.3) / balanced (1.0) / calm (0.7) multiplier
 *                   applied to base Motion durations
 * [accent2]       — secondary accent (highlight tint — accent2 from styleguide)
 * [accent3]       — tertiary accent (muted accent — accent3 from styleguide)
 */
@Immutable
data class LiquidTokens(
    val glassOpacity: Float,
    val glassBlurDp: Float,
    val saturation: Float,
    val glow: Float,
    val motionScale: Float,
    val accent2: Color,
    val accent3: Color,
)

// Motion profiles — maps to motionScale in LiquidTokens
object MotionProfile {
    /** Calm — 0.7× (snappy, tight). */
    const val Calm = 0.7f
    /** Balanced — 1.0× (the existing Motion.* values as-is). */
    const val Balanced = 1.0f
    /** Cinematic — 1.3× (grander, slower; default for Graphite Mist per c48e). */
    const val Cinematic = 1.3f
}

// ---------------------------------------------------------------------------
// AuroraDef — per-palette aurora blob parameterization
// ---------------------------------------------------------------------------

/**
 * Per-palette aurora definition. Carries the two primary glow colors (glowA,
 * glowB), plus the base canvas gradient colors (bg0→bg2). The [auroraCanvas]
 * Modifier reads the active palette's AuroraDef instead of hardcoded AURORA_*
 * constants, so each palette gets matching background mood.
 *
 * [bg0] is the darkest canvas tone; [bg2] the lightest (dark palettes invert this
 * convention for light palettes where bg0 is the lightest window canvas).
 * [glowA]/[glowB] drive the first two (largest, most prominent) radial blobs;
 * [overlayAccent] drives the mid-canvas E blob (accent/style-defining color).
 */
@Immutable
data class AuroraDef(
    val bg0: Color,
    val bg1: Color,
    val bg2: Color,
    val glowA: Color,         // primary blob color
    val glowB: Color,         // secondary blob color
    val overlayAccent: Color, // mid-canvas E blob color
    val overlayWarm: Color,   // mid-canvas F blob color (warm tone / amber)
    // CopyPaste-bdac.43: per-palette glass fill base color (--surface-rgb on web).
    // Used by LiquidGlassSurface as the dark-mode fill instead of the global GlassFillDark.
    // Light palettes keep Color(0xFFFFFFFF) (same as GlassFillLight).
    val surfaceRgb: Color,
)

// ---------------------------------------------------------------------------
// Per-palette token bundles
// ---------------------------------------------------------------------------

// ── Balanced-dark contrast profile helper ────────────────────────────────
// Source: /tmp/sg/source.html §2606–2607
// text .96, dim .78, faint .58, mute .46, icon .92, line white@.14, border @.16
// (ghost/ghostDeco are computed relative to text alpha in the web styleguide;
//  we use white@.62 / white@.46 as balanced-dark equivalents, same as DS-v2 dark)

private fun darkText(a: Float) = Color(0xFFF8FAFF).copy(alpha = a)
private fun darkTextBlue(a: Float) = Color(0xFFE5ECFF).copy(alpha = a)
private fun darkIcon(a: Float) = Color(0xFFF5F9FF).copy(alpha = a)
private fun darkLine(a: Float) = Color.White.copy(alpha = a)

// ── Balanced-light contrast profile helper ───────────────────────────────
// Source: §2620–2621
// text rgba(13,20,32,.92), dim rgba(35,47,67,.76), faint rgba(52,67,90,.58),
// mute rgba(70,86,111,.40), icon rgba(13,20,32,.88), line rgba(20,30,48,.14)

private fun lightText(r: Int, g: Int, b: Int, a: Float) = Color(r / 255f, g / 255f, b / 255f, a)

// ---------------------------------------------------------------------------
// GRAPHITE MIST (dark, grey) — the default palette (c48e)
// Exact values from task description + styleguide §2526–2531
// bg0 #07090f, bg1 #141924, bg2 #202330, glowA #7f8da3, glowB #5a6a83
// surface rgb(28,31,42), surfaceStrong rgb(42,46,60)
// accent #9db7df, accent2 #d5e2f7, accent3 #7c8ca6
// success #7be0b1, warning #ffcc6a, danger #ff7f8c, onAccent #ffffff
// contrast: balanced-dark (text @.96, dim @.78, faint @.58, mute @.46)
// glass opacity .64 / blur 28dp / saturation 1.45 / glow .62
// motion = cinematic (1.3×)
// ---------------------------------------------------------------------------

private val GmBg0      = Color(0xFF07090F)  // root window / darkest
private val GmBg1      = Color(0xFF141924)  // panel (bg1)
private val GmBg2      = Color(0xFF202330)  // elevated (bg2)
// surface rgb(28,31,42) → slightly lighter than bg2 for glass fill warmth
// surfaceStrong rgb(42,46,60) → raised / hover state
private val GmRaised   = Color(0xFF2A2E3C)  // rgb(42,46,60)

private val GmAccent        = Color(0xFF9DB7DF)
private val GmAccent2       = Color(0xFFD5E2F7)
private val GmAccent3       = Color(0xFF7C8CA6)
private val GmAccentPress   = Color(0xFF8AABDA)  // slightly deeper than accent for press

private val GmSuccess    = Color(0xFF7BE0B1)
private val GmWarning    = Color(0xFFFFCC6A)
private val GmDanger     = Color(0xFFFF7F8C)
private val GmInfo       = Color(0xFF5AB2C4)  // teal-adjacent (parity §A: CSS graphite-mist dark info)
private val GmViolet     = Color(0xFFB482D7)  // muted lavender (parity §A: CSS graphite-mist dark violet)

// Balanced-dark text/border tokens
// text #F8FAFF@.96, dim #E5ECFF@.78, faint/muted #D9E1F4@.58/icon #F5F9FF@.92
// line white@.14, border white@.16
private val GmText    = darkText(0.96f)
private val GmDim     = darkTextBlue(0.78f)
private val GmFaint   = Color(0xFFD9E1F4).copy(alpha = 0.58f)
private val GmMute    = Color(0xFFD9E1F4).copy(alpha = 0.46f)
private val GmIcon    = darkIcon(0.92f)  // used as ghost
private val GmGhost   = Color.White.copy(alpha = 0.55f)  // slightly brighter than DS-v2 dark to match cooler tone
private val GmGhostDeco = Color.White.copy(alpha = 0.38f)

val GraphiteMistIdeColors = IdeColors(
    bg       = GmBg0,
    panel    = GmBg1,
    elevated = GmBg2,
    raised   = GmRaised,
    border   = darkLine(0.16f),   // white@.16 per balanced-dark border
    divider  = darkLine(0.14f),   // white@.14 per balanced-dark line
    text     = GmText,
    dim      = GmDim,
    faint    = GmFaint,
    mute     = GmMute,
    ghost    = GmGhost,
    ghostDeco = GmGhostDeco,
    accent      = GmAccent,
    // #080C16 achieves 9.03:1 on GmAccent #9DB7DF; white was 2.04:1 (WCAG AA fail).
    accentOn    = Color(0xFF080C16),
    accentDim   = GmAccent.copy(alpha = 0.15f),
    accentPress = GmAccentPress,
    selection   = GmAccent.copy(alpha = 0.16f),
    hover       = Color.White.copy(alpha = 0.05f),
    success     = GmSuccess,
    successDim  = GmSuccess.copy(alpha = 0.12f),
    warning     = GmWarning,
    warningDim  = GmWarning.copy(alpha = 0.12f),
    danger      = GmDanger,
    dangerDim   = GmDanger.copy(alpha = 0.12f),
    info        = GmInfo,
    infoDim     = GmInfo.copy(alpha = 0.14f),
    violet      = GmViolet,
    violetDim   = GmViolet.copy(alpha = 0.14f),
)

val GraphiteMistLiquidTokens = LiquidTokens(
    glassOpacity  = 0.64f,
    glassBlurDp   = 28f,
    saturation    = 1.45f,   // styleguide Graphite Mist: subtler saturation than blue palettes
    glow          = 0.62f,
    motionScale   = MotionProfile.Cinematic,  // grander, slower per c48e spec
    accent2       = GmAccent2,
    accent3       = GmAccent3,
)

val GraphiteMistAurora = AuroraDef(
    bg0           = GmBg0,  // #07090f
    bg1           = GmBg1,  // #141924
    bg2           = GmBg2,  // #202330
    glowA         = Color(0xFF7F8DA3).copy(alpha = 0.45f),  // glowA #7f8da3 — cool grey-blue, main fill
    glowB         = Color(0xFF5A6A83).copy(alpha = 0.38f),  // glowB #5a6a83 — deeper slate
    overlayAccent = GmAccent.copy(alpha = 0.22f),           // accent-tinted mid-canvas E blob
    overlayWarm   = Color(0xFF5A6A83).copy(alpha = 0.14f),  // no amber in graphite palette; use glowB as warm stand-in
    surfaceRgb    = Color(0xFF1C1F2A),  // rgb(28,31,42) — styleguide surface token (bdac.43)
)

// ---------------------------------------------------------------------------
// LIQUID BLUE — existing Design-System-v2 dark ramp (mapped from DarkIdeColors)
// bg0 #061123, bg1 #0b1f47, bg2 #12152d, glowA #4d8dff, glowB #9e7bff
// accent #4d8dff, accent2 #7fc7ff, accent3 #9e7bff
// ---------------------------------------------------------------------------

val LiquidBlueLiquidTokens = LiquidTokens(
    glassOpacity  = 0.55f,   // existing GLASS_ALPHA_DARK
    glassBlurDp   = 28f,
    saturation    = 1.80f,   // full saturation (original DS-v2 recipe)
    glow          = 0.50f,
    motionScale   = MotionProfile.Balanced,
    accent2       = Color(0xFF7FC7FF),
    accent3       = Color(0xFF9E7BFF),
)

val LiquidBlueAurora = AuroraDef(
    bg0           = Color(0xFF061123),
    bg1           = Color(0xFF0B1F47),
    bg2           = Color(0xFF12152D),
    glowA         = Color(0xFF4D8DFF).copy(alpha = 0.42f),
    glowB         = Color(0xFF9E7BFF).copy(alpha = 0.38f),
    overlayAccent = Color(0xFF4D8DFF).copy(alpha = 0.22f),
    overlayWarm   = Color(0xFFD9A343).copy(alpha = 0.16f),
    surfaceRgb    = Color(0xFF161E36),  // rgb(22,30,54) — styleguide surface token (bdac.43)
)

private val LbBg0    = Color(0xFF061123)
private val LbBg1    = Color(0xFF0B1F47)
private val LbBg2    = Color(0xFF12152D)
private val LbAccent = Color(0xFF4D8DFF)

// CopyPaste-5917.38: bespoke IdeColors ramp for LiquidBlue — bg/panel/elevated match
// the AuroraDef so glass-surface backgrounds are in the same deep-blue family as the
// aurora canvas. Semantic colours match DeepSky profile tuned for #061123 background.
val LiquidBlueIdeColors = IdeColors(
    bg = LbBg0, panel = LbBg1, elevated = LbBg2,
    raised   = Color(0xFF1A2550),    // surfaceStrong — slightly lighter deep navy
    border   = darkLine(0.16f), divider = darkLine(0.14f),
    text     = darkText(0.96f), dim = darkTextBlue(0.78f),
    faint    = Color(0xFFD9E1F4).copy(alpha = 0.58f),
    mute     = Color(0xFFD9E1F4).copy(alpha = 0.46f),
    ghost    = Color.White.copy(alpha = 0.55f),
    ghostDeco = Color.White.copy(alpha = 0.38f),
    accent      = LbAccent,
    accentOn    = Color(0xFF080C16),  // dark text on blue: 6.11:1 on #4D8DFF (WCAG AA)
    accentDim   = LbAccent.copy(alpha = 0.15f),
    accentPress = Color(0xFF2F7AE8),
    selection   = LbAccent.copy(alpha = 0.16f),
    hover       = Color.White.copy(alpha = 0.05f),
    success     = Color(0xFF6EE79E),  successDim = Color(0xFF6EE79E).copy(alpha = 0.12f),
    warning     = Color(0xFFFFC75D),  warningDim = Color(0xFFFFC75D).copy(alpha = 0.12f),
    danger      = Color(0xFFFF6B78),  dangerDim  = Color(0xFFFF6B78).copy(alpha = 0.12f),
    info        = Color(0xFF7FC7FF),  infoDim    = Color(0xFF7FC7FF).copy(alpha = 0.14f),
    violet      = Color(0xFF9E7BFF),  violetDim  = Color(0xFF9E7BFF).copy(alpha = 0.14f),
)

// ---------------------------------------------------------------------------
// DEEP SKY (dark, electric blue)
// bg0 #021222, bg1 #073766, bg2 #101b31, glowA #1fa4ff, glowB #55e6ff
// accent #1f9cff, accent2 #8cddff, accent3 #4d74ff
// success #6ee79e, warning #ffc75d, danger #ff6b78
// ---------------------------------------------------------------------------

private val DsBg0   = Color(0xFF021222)
private val DsBg1   = Color(0xFF073766)
private val DsBg2   = Color(0xFF101B31)
private val DsAccent = Color(0xFF1F9CFF)

val DeepSkyIdeColors = IdeColors(
    bg       = DsBg0, panel = DsBg1, elevated = DsBg2,
    raised   = Color(0xFF183052),    // surfaceStrong rgb(24,48,82) from styleguide
    border   = darkLine(0.16f), divider = darkLine(0.14f),
    text     = darkText(0.96f), dim = darkTextBlue(0.78f),
    faint    = Color(0xFFD9E1F4).copy(alpha = 0.58f),
    mute     = Color(0xFFD9E1F4).copy(alpha = 0.46f),
    ghost    = Color.White.copy(alpha = 0.55f),
    ghostDeco = Color.White.copy(alpha = 0.38f),
    accent      = DsAccent,
    // #080C16 achieves 6.75:1 on DsAccent #1F9CFF; white was 2.89:1 (WCAG AA fail).
    accentOn    = Color(0xFF080C16),
    accentDim   = DsAccent.copy(alpha = 0.15f),
    accentPress = Color(0xFF0E8FF0),
    selection   = DsAccent.copy(alpha = 0.16f),
    hover       = Color.White.copy(alpha = 0.05f),
    success     = Color(0xFF6EE79E),  successDim = Color(0xFF6EE79E).copy(alpha = 0.12f),
    warning     = Color(0xFFFFC75D),  warningDim = Color(0xFFFFC75D).copy(alpha = 0.12f),
    danger      = Color(0xFFFF6B78),  dangerDim  = Color(0xFFFF6B78).copy(alpha = 0.12f),
    info        = Color(0xFF28C3E4),  infoDim    = Color(0xFF28C3E4).copy(alpha = 0.14f),  // parity §A: CSS deep-sky dark info
    violet      = Color(0xFF8C6CE8),  violetDim  = Color(0xFF8C6CE8).copy(alpha = 0.14f),  // parity §A: CSS deep-sky dark violet
)

val DeepSkyLiquidTokens = LiquidTokens(
    glassOpacity = 0.58f, glassBlurDp = 28f, saturation = 1.80f, glow = 0.55f,
    motionScale  = MotionProfile.Balanced,
    accent2 = Color(0xFF8CDDFF), accent3 = Color(0xFF4D74FF),
)

val DeepSkyAurora = AuroraDef(
    bg0 = DsBg0, bg1 = DsBg1, bg2 = DsBg2,
    glowA         = Color(0xFF1FA4FF).copy(alpha = 0.45f),
    glowB         = Color(0xFF55E6FF).copy(alpha = 0.35f),
    overlayAccent = Color(0xFF1F9CFF).copy(alpha = 0.22f),
    overlayWarm   = Color(0xFFD9A343).copy(alpha = 0.14f),
    surfaceRgb    = Color(0xFF0E1B31),  // rgb(14,27,49) — between bg2 #101B31 and raised #183052 (bdac.43)
)

// ---------------------------------------------------------------------------
// NORDIC CYAN (dark, grey-blue teal)
// bg0 #031216, bg1 #06333b, bg2 #0b1d2b, glowA #24d6b5, glowB #478cff
// accent #25d5b4, accent2 #8af5e4, accent3 #5a9dff
// success #75ef9f, warning #ffd166, danger #ff6b6b
// ---------------------------------------------------------------------------

private val NcBg0    = Color(0xFF031216)
private val NcBg1    = Color(0xFF06333B)
private val NcBg2    = Color(0xFF0B1D2B)
private val NcAccent = Color(0xFF25D5B4)

val NordicCyanIdeColors = IdeColors(
    bg = NcBg0, panel = NcBg1, elevated = NcBg2,
    raised   = Color(0xFF153746),  // rgb(21,55,70)
    border   = darkLine(0.16f), divider = darkLine(0.14f),
    text     = darkText(0.96f), dim = darkTextBlue(0.78f),
    faint    = Color(0xFFD9E1F4).copy(alpha = 0.58f),
    mute     = Color(0xFFD9E1F4).copy(alpha = 0.46f),
    ghost    = Color.White.copy(alpha = 0.55f),
    ghostDeco = Color.White.copy(alpha = 0.38f),
    accent      = NcAccent,
    accentOn    = Color(0xFF031216),  // dark text on teal for contrast
    accentDim   = NcAccent.copy(alpha = 0.15f),
    accentPress = Color(0xFF1CC5A5),
    selection   = NcAccent.copy(alpha = 0.16f),
    hover       = Color.White.copy(alpha = 0.05f),
    success     = Color(0xFF75EF9F),  successDim = Color(0xFF75EF9F).copy(alpha = 0.12f),
    warning     = Color(0xFFFFD166),  warningDim = Color(0xFFFFD166).copy(alpha = 0.12f),
    danger      = Color(0xFFFF6B6B),  dangerDim  = Color(0xFFFF6B6B).copy(alpha = 0.12f),
    info        = Color(0xFF25D5B4),  infoDim    = Color(0xFF25D5B4).copy(alpha = 0.14f),  // parity §A: CSS nordic-cyan dark info (tracks teal accent)
    violet      = Color(0xFFA078F0),  violetDim  = Color(0xFFA078F0).copy(alpha = 0.14f),  // parity §A: CSS nordic-cyan dark violet
)

val NordicCyanLiquidTokens = LiquidTokens(
    glassOpacity = 0.58f, glassBlurDp = 28f, saturation = 1.70f, glow = 0.58f,
    motionScale  = MotionProfile.Balanced,
    accent2 = Color(0xFF8AF5E4), accent3 = Color(0xFF5A9DFF),
)

val NordicCyanAurora = AuroraDef(
    bg0 = NcBg0, bg1 = NcBg1, bg2 = NcBg2,
    glowA         = Color(0xFF24D6B5).copy(alpha = 0.42f),
    glowB         = Color(0xFF478CFF).copy(alpha = 0.32f),
    overlayAccent = Color(0xFF25D5B4).copy(alpha = 0.20f),
    overlayWarm   = Color(0xFFFFD166).copy(alpha = 0.14f),
    surfaceRgb    = Color(0xFF0D2130),  // rgb(13,33,48) — close to bg2 #0B1D2B (bdac.43)
)

// ---------------------------------------------------------------------------
// AURORA VIOLET (dark, purple/pink)
// bg0 #11071f, bg1 #28114d, bg2 #14172d, glowA #9a7cff, glowB #ff7ad9
// accent #9a7cff, accent2 #d4b6ff, accent3 #ff7ad9
// success #6ee7b7, warning #ffc35a, danger #ff6f91
// ---------------------------------------------------------------------------

private val AvBg0    = Color(0xFF11071F)
private val AvBg1    = Color(0xFF28114D)
private val AvBg2    = Color(0xFF14172D)
private val AvAccent = Color(0xFF9A7CFF)

val AuroraVioletIdeColors = IdeColors(
    bg = AvBg0, panel = AvBg1, elevated = AvBg2,
    raised   = Color(0xFF302452),  // rgb(48,36,82)
    border   = darkLine(0.16f), divider = darkLine(0.14f),
    text     = darkText(0.96f), dim = darkTextBlue(0.78f),
    faint    = Color(0xFFD9E1F4).copy(alpha = 0.58f),
    mute     = Color(0xFFD9E1F4).copy(alpha = 0.46f),
    ghost    = Color.White.copy(alpha = 0.55f),
    ghostDeco = Color.White.copy(alpha = 0.38f),
    accent      = AvAccent,
    // #080C16 achieves 6.24:1 on AvAccent #9A7CFF; white was 3.13:1 (WCAG AA fail).
    accentOn    = Color(0xFF080C16),
    accentDim   = AvAccent.copy(alpha = 0.15f),
    accentPress = Color(0xFF8A6DF5),
    selection   = AvAccent.copy(alpha = 0.16f),
    hover       = Color.White.copy(alpha = 0.05f),
    success     = Color(0xFF6EE7B7),  successDim = Color(0xFF6EE7B7).copy(alpha = 0.12f),
    warning     = Color(0xFFFFC35A),  warningDim = Color(0xFFFFC35A).copy(alpha = 0.12f),
    danger      = Color(0xFFFF6F91),  dangerDim  = Color(0xFFFF6F91).copy(alpha = 0.12f),
    info        = Color(0xFFAA80FF),  infoDim    = Color(0xFFAA80FF).copy(alpha = 0.14f),  // parity §A: CSS aurora-violet dark info (violet-shifted)
    violet      = Color(0xFF9A7CFF),  violetDim  = Color(0xFF9A7CFF).copy(alpha = 0.14f),  // parity §A: CSS aurora-violet dark violet (tracks accent)
)

val AuroraVioletLiquidTokens = LiquidTokens(
    glassOpacity = 0.60f, glassBlurDp = 32f, saturation = 1.90f, glow = 0.68f,
    motionScale  = MotionProfile.Cinematic,
    accent2 = Color(0xFFD4B6FF), accent3 = Color(0xFFFF7AD9),
)

val AuroraVioletAurora = AuroraDef(
    bg0 = AvBg0, bg1 = AvBg1, bg2 = AvBg2,
    glowA         = Color(0xFF9A7CFF).copy(alpha = 0.45f),
    glowB         = Color(0xFFFF7AD9).copy(alpha = 0.38f),
    overlayAccent = Color(0xFF9A7CFF).copy(alpha = 0.22f),
    overlayWarm   = Color(0xFFFFC35A).copy(alpha = 0.16f),
    surfaceRgb    = Color(0xFF1D1836),  // rgb(29,24,54) — styleguide surface token (bdac.43)
)

// ---------------------------------------------------------------------------
// AMBER NIGHT (dark, warm amber+blue)
// bg0 #171008, bg1 #3a220a, bg2 #1c1a22, glowA #ff9f1c, glowB #6ca0ff
// accent #ffad33, accent2 #ffd28a, accent3 #6ca0ff
// success #82e070, warning #ffbf47, danger #ff6b68
// ---------------------------------------------------------------------------

private val AnBg0    = Color(0xFF171008)
private val AnBg1    = Color(0xFF3A220A)
private val AnBg2    = Color(0xFF1C1A22)
private val AnAccent = Color(0xFFFFAD33)

val AmberNightIdeColors = IdeColors(
    bg = AnBg0, panel = AnBg1, elevated = AnBg2,
    raised   = Color(0xFF3A2B1E),  // rgb(58,43,30)
    border   = darkLine(0.16f), divider = darkLine(0.14f),
    text     = darkText(0.96f), dim = darkTextBlue(0.78f),
    faint    = Color(0xFFD9E1F4).copy(alpha = 0.58f),
    mute     = Color(0xFFD9E1F4).copy(alpha = 0.46f),
    ghost    = Color.White.copy(alpha = 0.55f),
    ghostDeco = Color.White.copy(alpha = 0.38f),
    accent      = AnAccent,
    accentOn    = Color(0xFF1A0D00),  // very dark on amber
    accentDim   = AnAccent.copy(alpha = 0.15f),
    accentPress = Color(0xFFF0A030),
    selection   = AnAccent.copy(alpha = 0.16f),
    hover       = Color.White.copy(alpha = 0.05f),
    success     = Color(0xFF82E070),  successDim = Color(0xFF82E070).copy(alpha = 0.12f),
    warning     = Color(0xFFFFBF47),  warningDim = Color(0xFFFFBF47).copy(alpha = 0.12f),
    danger      = Color(0xFFFF6B68),  dangerDim  = Color(0xFFFF6B68).copy(alpha = 0.12f),
    info        = Color(0xFF64A0B9),  infoDim    = Color(0xFF64A0B9).copy(alpha = 0.14f),  // parity §A: CSS amber-night dark info (warm-tinted teal)
    violet      = Color(0xFFBE82C8),  violetDim  = Color(0xFFBE82C8).copy(alpha = 0.14f),  // parity §A: CSS amber-night dark violet (muted warm-purple)
)

val AmberNightLiquidTokens = LiquidTokens(
    glassOpacity = 0.60f, glassBlurDp = 28f, saturation = 1.70f, glow = 0.60f,
    motionScale  = MotionProfile.Balanced,
    accent2 = Color(0xFFFFD28A), accent3 = Color(0xFF6CA0FF),
)

val AmberNightAurora = AuroraDef(
    bg0 = AnBg0, bg1 = AnBg1, bg2 = AnBg2,
    glowA         = Color(0xFFFF9F1C).copy(alpha = 0.42f),
    glowB         = Color(0xFF6CA0FF).copy(alpha = 0.32f),
    overlayAccent = Color(0xFFFFAD33).copy(alpha = 0.22f),
    overlayWarm   = Color(0xFFFF9F1C).copy(alpha = 0.18f),
    surfaceRgb    = Color(0xFF241E22),  // rgb(36,30,34) — warm-amber tinted surface (bdac.43)
)

// ---------------------------------------------------------------------------
// CLOUD SILVER (light, cool grey) — balanced-light contrast profile
// bg0 #edf2f8, bg1 #dce6f3, bg2 #f8fbff
// accent #5b8def, accent2 #2f74e8, accent3 #9cb3d7
// success #158f48, warning #a86700, danger #d93b4a
// ---------------------------------------------------------------------------

private val CsAccent = Color(0xFF5B8DEF)

val CloudSilverIdeColors = IdeColors(
    bg       = Color(0xFFEDF2F8), panel = Color(0xFFDCE6F3), elevated = Color(0xFFF8FBFF),
    raised   = Color(0xFFFFFFFF),   // surfaceStrong = white for light
    border   = lightText(20, 30, 48, 0.16f), divider = lightText(20, 30, 48, 0.14f),
    text     = lightText(13, 20, 32, 0.92f),
    dim      = lightText(35, 47, 67, 0.76f),
    faint    = lightText(52, 67, 90, 0.58f),
    mute     = lightText(70, 86, 111, 0.46f),
    ghost    = Color(0xFF3C3C43).copy(alpha = 0.55f),
    ghostDeco = Color(0xFF3C3C43).copy(alpha = 0.32f),
    accent      = CsAccent,
    // #080C16 achieves 6.05:1 on CsAccent #5B8DEF; white was 3.23:1 (WCAG AA fail).
    accentOn    = Color(0xFF080C16),
    accentDim   = CsAccent.copy(alpha = 0.12f),
    accentPress = Color(0xFF4A7EDF),
    selection   = CsAccent.copy(alpha = 0.14f),
    hover       = Color.Black.copy(alpha = 0.04f),
    success     = Color(0xFF158F48),  successDim = Color(0xFF158F48).copy(alpha = 0.12f),
    warning     = Color(0xFFA86700),  warningDim = Color(0xFFA86700).copy(alpha = 0.12f),
    danger      = Color(0xFFD93B4A),  dangerDim  = Color(0xFFD93B4A).copy(alpha = 0.12f),
    info        = Color(0xFF0F6E9B),  infoDim    = Color(0xFF0F6E9B).copy(alpha = 0.12f),  // parity §A: CSS cloud-silver light info
    violet      = Color(0xFF6E50BE),  violetDim  = Color(0xFF6E50BE).copy(alpha = 0.12f),  // parity §A: CSS cloud-silver light violet
)

val CloudSilverLiquidTokens = LiquidTokens(
    glassOpacity = 0.62f, glassBlurDp = 28f, saturation = 1.60f, glow = 0.40f,
    motionScale  = MotionProfile.Balanced,
    accent2 = Color(0xFF2F74E8), accent3 = Color(0xFF9CB3D7),
)

val CloudSilverAurora = AuroraDef(
    bg0 = Color(0xFFEDF2F8), bg1 = Color(0xFFDCE6F3), bg2 = Color(0xFFF8FBFF),
    glowA         = Color(0xFFB9C7D9).copy(alpha = 0.25f),  // glowA — soft cool grey
    glowB         = Color(0xFFDCE7F7).copy(alpha = 0.20f),  // glowB — near-white blue
    overlayAccent = Color(0xFF5B8DEF).copy(alpha = 0.12f),  // accent E blob (light)
    overlayWarm   = Color(0xFFD9A343).copy(alpha = 0.08f),  // subtle warm F blob
    surfaceRgb    = Color(0xFFFFFFFF),  // light palette: pure white fill (same as GlassFillLight; bdac.43)
)

// ---------------------------------------------------------------------------
// FROST BLUE (light, icy blue)
// bg0 #edf7ff, bg1 #d9ecff, bg2 #f8fcff
// accent #2777ff, accent2 #005fe3, accent3 #72b8ff
// success #108747, warning #9a6200, danger #ca3446
// ---------------------------------------------------------------------------

private val FbAccent = Color(0xFF2777FF)

val FrostBlueIdeColors = IdeColors(
    bg       = Color(0xFFEDF7FF), panel = Color(0xFFD9ECFF), elevated = Color(0xFFF8FCFF),
    raised   = Color(0xFFFFFFFF),
    border   = lightText(20, 30, 48, 0.16f), divider = lightText(20, 30, 48, 0.14f),
    text     = lightText(13, 20, 32, 0.92f),
    dim      = lightText(35, 47, 67, 0.76f),
    faint    = lightText(52, 67, 90, 0.58f),
    mute     = lightText(70, 86, 111, 0.46f),
    ghost    = Color(0xFF3C3C43).copy(alpha = 0.55f),
    ghostDeco = Color(0xFF3C3C43).copy(alpha = 0.32f),
    accent      = FbAccent,
    // #080C16 achieves 4.81:1 on FbAccent #2777FF; white was 4.06:1 (WCAG AA fail).
    accentOn    = Color(0xFF080C16),
    accentDim   = FbAccent.copy(alpha = 0.12f),
    accentPress = Color(0xFF1A6DF0),
    selection   = FbAccent.copy(alpha = 0.14f),
    hover       = Color.Black.copy(alpha = 0.04f),
    success     = Color(0xFF108747),  successDim = Color(0xFF108747).copy(alpha = 0.12f),
    warning     = Color(0xFF9A6200),  warningDim = Color(0xFF9A6200).copy(alpha = 0.12f),
    danger      = Color(0xFFCA3446),  dangerDim  = Color(0xFFCA3446).copy(alpha = 0.12f),
    info        = Color(0xFF0F6EA0),  infoDim    = Color(0xFF0F6EA0).copy(alpha = 0.12f),  // parity §A: CSS frost-blue light info
    violet      = Color(0xFF644BC3),  violetDim  = Color(0xFF644BC3).copy(alpha = 0.12f),  // parity §A: CSS frost-blue light violet
)

val FrostBlueLiquidTokens = LiquidTokens(
    glassOpacity = 0.62f, glassBlurDp = 28f, saturation = 1.60f, glow = 0.40f,
    motionScale  = MotionProfile.Balanced,
    accent2 = Color(0xFF005FE3), accent3 = Color(0xFF72B8FF),
)

val FrostBlueAurora = AuroraDef(
    bg0 = Color(0xFFEDF7FF), bg1 = Color(0xFFD9ECFF), bg2 = Color(0xFFF8FCFF),
    glowA         = Color(0xFF91C9FF).copy(alpha = 0.30f),
    glowB         = Color(0xFFC7E6FF).copy(alpha = 0.22f),
    overlayAccent = Color(0xFF2777FF).copy(alpha = 0.12f),
    overlayWarm   = Color(0xFFD9A343).copy(alpha = 0.08f),
    surfaceRgb    = Color(0xFFFFFFFF),  // light palette: pure white fill (bdac.43)
)

// ---------------------------------------------------------------------------
// PORCELAIN (light, grey-blue)
// bg0 #f3f6fa, bg1 #e4ebf4, bg2 #fbfdff
// accent #3c7dd9, accent2 #1e5fb9, accent3 #8fb8e8
// success #16874c, warning #9b6500, danger #ca3446
// ---------------------------------------------------------------------------

private val PorcAccent = Color(0xFF3C7DD9)

val PorcelainIdeColors = IdeColors(
    bg       = Color(0xFFF3F6FA), panel = Color(0xFFE4EBF4), elevated = Color(0xFFFBFDFF),
    raised   = Color(0xFFFFFFFF),
    border   = lightText(20, 30, 48, 0.16f), divider = lightText(20, 30, 48, 0.14f),
    text     = lightText(13, 20, 32, 0.92f),
    dim      = lightText(35, 47, 67, 0.76f),
    faint    = lightText(52, 67, 90, 0.58f),
    mute     = lightText(70, 86, 111, 0.46f),
    ghost    = Color(0xFF3C3C43).copy(alpha = 0.55f),
    ghostDeco = Color(0xFF3C3C43).copy(alpha = 0.32f),
    accent      = PorcAccent,
    // #080C16 achieves 4.77:1 on PorcAccent #3C7DD9; white was 4.10:1 (WCAG AA fail).
    accentOn    = Color(0xFF080C16),
    accentDim   = PorcAccent.copy(alpha = 0.12f),
    accentPress = Color(0xFF2E6FC5),
    selection   = PorcAccent.copy(alpha = 0.14f),
    hover       = Color.Black.copy(alpha = 0.04f),
    success     = Color(0xFF16874C),  successDim = Color(0xFF16874C).copy(alpha = 0.12f),
    warning     = Color(0xFF9B6500),  warningDim = Color(0xFF9B6500).copy(alpha = 0.12f),
    danger      = Color(0xFFCA3446),  dangerDim  = Color(0xFFCA3446).copy(alpha = 0.12f),
    info        = Color(0xFF1273A5),  infoDim    = Color(0xFF1273A5).copy(alpha = 0.12f),  // parity §A: CSS porcelain light info (within ±5 of CSS 18/115/165)
    violet      = Color(0xFF6950C3),  violetDim  = Color(0xFF6950C3).copy(alpha = 0.12f),  // parity §A: CSS porcelain light violet
)

val PorcelainLiquidTokens = LiquidTokens(
    glassOpacity = 0.62f, glassBlurDp = 28f, saturation = 1.60f, glow = 0.40f,
    motionScale  = MotionProfile.Balanced,
    accent2 = Color(0xFF1E5FB9), accent3 = Color(0xFF8FB8E8),
)

val PorcelainAurora = AuroraDef(
    bg0 = Color(0xFFF3F6FA), bg1 = Color(0xFFE4EBF4), bg2 = Color(0xFFFBFDFF),
    glowA         = Color(0xFFA9C9EF).copy(alpha = 0.28f),
    glowB         = Color(0xFFD2E5FB).copy(alpha = 0.20f),
    overlayAccent = Color(0xFF3C7DD9).copy(alpha = 0.12f),
    overlayWarm   = Color(0xFFD9A343).copy(alpha = 0.08f),
    surfaceRgb    = Color(0xFFFFFFFF),  // light palette: pure white fill (bdac.43)
)

// ---------------------------------------------------------------------------
// PEARL GREY (light, neutral grey)
// bg0 #f1f1f2, bg1 #dedfe3, bg2 #fafafa
// accent #58677f, accent2 #34465f, accent3 #9ba6b7
// success #1c8f50, warning #9b6400, danger #c93445
// ---------------------------------------------------------------------------

private val PearlAccent = Color(0xFF58677F)

val PearlGreyIdeColors = IdeColors(
    bg       = Color(0xFFF1F1F2), panel = Color(0xFFDEDFE3), elevated = Color(0xFFFAFAFA),
    raised   = Color(0xFFFFFFFF),
    border   = lightText(20, 30, 48, 0.16f), divider = lightText(20, 30, 48, 0.14f),
    text     = lightText(13, 20, 32, 0.92f),
    dim      = lightText(35, 47, 67, 0.76f),
    faint    = lightText(52, 67, 90, 0.58f),
    mute     = lightText(70, 86, 111, 0.46f),
    ghost    = Color(0xFF3C3C43).copy(alpha = 0.55f),
    ghostDeco = Color(0xFF3C3C43).copy(alpha = 0.32f),
    accent      = PearlAccent,
    accentOn    = Color.White,
    accentDim   = PearlAccent.copy(alpha = 0.12f),
    accentPress = Color(0xFF4A5870),
    selection   = PearlAccent.copy(alpha = 0.14f),
    hover       = Color.Black.copy(alpha = 0.04f),
    success     = Color(0xFF1C8F50),  successDim = Color(0xFF1C8F50).copy(alpha = 0.12f),
    warning     = Color(0xFF9B6400),  warningDim = Color(0xFF9B6400).copy(alpha = 0.12f),
    danger      = Color(0xFFC93445),  dangerDim  = Color(0xFFC93445).copy(alpha = 0.12f),
    info        = Color(0xFF1478AA),  infoDim    = Color(0xFF1478AA).copy(alpha = 0.12f),
    violet      = Color(0xFF7358C8),  violetDim  = Color(0xFF7358C8).copy(alpha = 0.12f),  // parity §A: CSS pearl-grey light violet
)

val PearlGreyLiquidTokens = LiquidTokens(
    glassOpacity = 0.62f, glassBlurDp = 28f, saturation = 1.50f, glow = 0.35f,
    motionScale  = MotionProfile.Balanced,
    accent2 = Color(0xFF34465F), accent3 = Color(0xFF9BA6B7),
)

val PearlGreyAurora = AuroraDef(
    bg0 = Color(0xFFF1F1F2), bg1 = Color(0xFFDEDFE3), bg2 = Color(0xFFFAFAFA),
    glowA         = Color(0xFFAEB4BD).copy(alpha = 0.25f),
    glowB         = Color(0xFFD4D6DC).copy(alpha = 0.18f),
    overlayAccent = Color(0xFF58677F).copy(alpha = 0.10f),
    overlayWarm   = Color(0xFFD9A343).copy(alpha = 0.06f),
    surfaceRgb    = Color(0xFFFFFFFF),  // light palette: pure white fill (bdac.43)
)

// ---------------------------------------------------------------------------
// Palette → ramp resolver functions
// ---------------------------------------------------------------------------

/**
 * Returns the [IdeColors] ramp for [palette].
 * Screens read [LocalIdeColors] — this function wires the enum to the ramp
 * inside [CopyPasteTheme].
 */
fun paletteIdeColors(palette: Palette): IdeColors = when (palette) {
    Palette.GRAPHITE_MIST  -> GraphiteMistIdeColors
    Palette.LIQUID_BLUE    -> LiquidBlueIdeColors    // CopyPaste-5917.38: bespoke deep-blue ramp
    Palette.DEEP_SKY       -> DeepSkyIdeColors
    Palette.NORDIC_CYAN    -> NordicCyanIdeColors
    Palette.AURORA_VIOLET  -> AuroraVioletIdeColors
    Palette.AMBER_NIGHT    -> AmberNightIdeColors
    Palette.CLOUD_SILVER   -> CloudSilverIdeColors
    Palette.FROST_BLUE     -> FrostBlueIdeColors
    Palette.PORCELAIN      -> PorcelainIdeColors
    Palette.PEARL_GREY     -> PearlGreyIdeColors
}

/**
 * Theme-aware ramp: NEUTRALS come from the dark/light base ramp, CHROMA (accent)
 * from the palette — so EVERY palette renders correctly in BOTH dark and light
 * (mirrors the web data-theme × data-palette split, CopyPaste-s0uf). A palette in
 * its native scheme reuses its hand-tuned ramp; off-scheme it gets the base
 * neutrals plus a contrast-tuned accent.
 */
fun paletteIdeColors(palette: Palette, dark: Boolean): IdeColors =
    if (dark) when (palette) {
        Palette.GRAPHITE_MIST -> GraphiteMistIdeColors
        Palette.LIQUID_BLUE   -> LiquidBlueIdeColors  // CopyPaste-5917.38
        Palette.DEEP_SKY      -> DeepSkyIdeColors
        Palette.NORDIC_CYAN   -> NordicCyanIdeColors
        Palette.AURORA_VIOLET -> AuroraVioletIdeColors
        Palette.AMBER_NIGHT   -> AmberNightIdeColors
        // Light palettes shown in dark mode: dark neutrals + brightened accent.
        Palette.CLOUD_SILVER  -> DarkIdeColors.withAccent(Color(0xFF7AABFF))
        Palette.FROST_BLUE    -> DarkIdeColors.withAccent(Color(0xFF5A96FF))
        Palette.PORCELAIN     -> DarkIdeColors.withAccent(Color(0xFF6AA3F5))
        Palette.PEARL_GREY    -> DarkIdeColors.withAccent(Color(0xFF8AA4CC))
    } else when (palette) {
        Palette.CLOUD_SILVER  -> CloudSilverIdeColors
        Palette.FROST_BLUE    -> FrostBlueIdeColors
        Palette.PORCELAIN     -> PorcelainIdeColors
        Palette.PEARL_GREY    -> PearlGreyIdeColors
        // Dark palettes shown in light mode: light neutrals + darkened accent (AA on light).
        Palette.GRAPHITE_MIST -> LightIdeColors.withAccent(Color(0xFF3A6091))
        Palette.LIQUID_BLUE   -> LightIdeColors.withAccent(Color(0xFF1A5FD4))
        Palette.DEEP_SKY      -> LightIdeColors.withAccent(Color(0xFF0070CC))
        Palette.NORDIC_CYAN   -> LightIdeColors.withAccent(Color(0xFF0A8F78))
        Palette.AURORA_VIOLET -> LightIdeColors.withAccent(Color(0xFF5A35C8))
        Palette.AMBER_NIGHT   -> LightIdeColors.withAccent(Color(0xFFA05D00))
    }

/** Overlay [accent] onto a base ramp's neutrals; accentDim/selection derived by alpha. */
private fun IdeColors.withAccent(accent: Color): IdeColors = copy(
    accent = accent,
    accentDim = accent.copy(alpha = 0.12f),
    accentPress = accent,
    selection = accent.copy(alpha = 0.16f),
)

/**
 * Returns the [LiquidTokens] for [palette].
 */
fun paletteLiquidTokens(palette: Palette): LiquidTokens = when (palette) {
    Palette.GRAPHITE_MIST  -> GraphiteMistLiquidTokens
    Palette.LIQUID_BLUE    -> LiquidBlueLiquidTokens
    Palette.DEEP_SKY       -> DeepSkyLiquidTokens
    Palette.NORDIC_CYAN    -> NordicCyanLiquidTokens
    Palette.AURORA_VIOLET  -> AuroraVioletLiquidTokens
    Palette.AMBER_NIGHT    -> AmberNightLiquidTokens
    Palette.CLOUD_SILVER   -> CloudSilverLiquidTokens
    Palette.FROST_BLUE     -> FrostBlueLiquidTokens
    Palette.PORCELAIN      -> PorcelainLiquidTokens
    Palette.PEARL_GREY     -> PearlGreyLiquidTokens
}

/**
 * Returns the [AuroraDef] for [palette].
 */
fun paletteAurora(palette: Palette): AuroraDef = when (palette) {
    Palette.GRAPHITE_MIST  -> GraphiteMistAurora
    Palette.LIQUID_BLUE    -> LiquidBlueAurora
    Palette.DEEP_SKY       -> DeepSkyAurora
    Palette.NORDIC_CYAN    -> NordicCyanAurora
    Palette.AURORA_VIOLET  -> AuroraVioletAurora
    Palette.AMBER_NIGHT    -> AmberNightAurora
    Palette.CLOUD_SILVER   -> CloudSilverAurora
    Palette.FROST_BLUE     -> FrostBlueAurora
    Palette.PORCELAIN      -> PorcelainAurora
    Palette.PEARL_GREY     -> PearlGreyAurora
}

// ---------------------------------------------------------------------------
// CompositionLocals
// ---------------------------------------------------------------------------

/**
 * Provides the active [Palette] enum value down the tree.
 * Screens that need to branch on palette identity (rare) read this;
 * most code reads [LocalIdeColors] or [LocalLiquidTokens].
 */
val LocalPalette = staticCompositionLocalOf { Palette.DEFAULT }

/**
 * Provides the active [LiquidTokens] (glass params / motion / accent2/3) down
 * the tree. Provided by [CopyPasteTheme] alongside [LocalIdeColors].
 * Defaults to Graphite Mist tokens — the new default palette.
 */
val LocalLiquidTokens = staticCompositionLocalOf<LiquidTokens> { GraphiteMistLiquidTokens }
