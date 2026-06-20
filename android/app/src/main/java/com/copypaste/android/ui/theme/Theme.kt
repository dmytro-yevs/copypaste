package com.copypaste.android.ui.theme

import android.app.Activity
import android.content.Context
import android.provider.Settings as AndroidSettings
import android.view.WindowManager
import android.view.accessibility.AccessibilityManager
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat
import com.copypaste.android.Settings
import com.copypaste.android.ThemeMode
import com.copypaste.android.rememberSkin

// ---------------------------------------------------------------------------
// CopyPaste theme — **light-first** (PARITY-SPEC §0), matching the Apple macOS
// Tahoe "Liquid Glass" palette in docs/PARITY-SPEC.md §1, mirrored in
// crates/copypaste-ui/src/index.css.
//
// The default theme is LIGHT (NOT follow-OS). A Settings control drives a
// [ThemeMode] (System / Light / Dark); CopyPasteTheme reads it from the
// persisted pref via [rememberThemeMode] so every screen follows the user's
// choice. Only ThemeMode.SYSTEM follows the OS dark/light setting.
//
// A dark colour scheme (DarculaColorScheme) is retained for Dark / System-dark.
//
// Dynamic color (Material You) is intentionally disabled: it would override
// the precise canonical palette we need to match the desktop app.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// §8 Motion tokens — mirrors CSS custom properties in index.css
//
//   instant  90ms   — press-scale feedback, copy flash
//   fast    130ms   — hover, list-item mount, selection glide
//   base    180ms   — standard transitions, toast
//   slow    240ms   — larger surface motions
//
// EaseOutExpo — CubicBezierEasing(0.16, 1.0, 0.3, 1.0) — the "spring-like"
// out-expo curve used for entrance animations and press scale.
// ---------------------------------------------------------------------------
object Motion {
    const val Instant = 90
    const val Fast    = 130
    const val Base    = 180
    const val Slow    = 240
}

// ---------------------------------------------------------------------------
// LocalIdeColors — the active full-token ramp, provided by CopyPasteTheme.
// Screens read `LocalIdeColors.current.<token>` instead of hardcoded constants.
// staticCompositionLocalOf: only changes on a full theme/palette switch
// (activity recreation). Defaults to Graphite Mist so any stray reader outside
// CopyPasteTheme is still defined with the new default palette (c48e).
// ---------------------------------------------------------------------------
val LocalIdeColors = staticCompositionLocalOf<IdeColors> { GraphiteMistIdeColors }

/** §8 out-expo easing — matches CSS cubic-bezier(.16,1,.3,1). */
val EaseOutExpo = CubicBezierEasing(0.16f, 1.0f, 0.3f, 1.0f)

/** §8 standard easing — matches CSS cubic-bezier(.2,0,0,1). */
val EaseStandard = CubicBezierEasing(0.20f, 0.0f, 0.0f, 1.0f)

/** §8 ease-in — matches CSS cubic-bezier(.4,0,1,1). */
val EaseIn = CubicBezierEasing(0.40f, 0.0f, 1.0f, 1.0f)

// ---------------------------------------------------------------------------
// §8 Reduced-motion gate — mirrors the web prefers-reduced-motion media query.
//
// Two signals are checked, either of which independently disables animations:
//   1. AccessibilityManager.isAnimationEnabled() == false
//      (covers "Remove animations" / "Disable animations" in Accessibility settings)
//   2. Settings.Global.ANIMATOR_DURATION_SCALE == 0f
//      (covers Developer Options → "Animator duration scale = off")
//
// Both APIs are available on API 26+ (our minSdk). The result is remembered
// across recompositions but is read fresh on each composition entry because the
// user may toggle these settings while the app is in the foreground.
// ---------------------------------------------------------------------------

/**
 * Returns `true` when the user has requested reduced motion via any of the
 * platform's animation-disable mechanisms.  When `true`, callers MUST skip or
 * shorten their animated transitions.
 */
@Composable
fun rememberReducedMotion(): Boolean {
    val context = LocalContext.current
    return remember(context) { isReducedMotion(context) }
}

/**
 * Non-composable helper — safe to call from helpers that already have a [Context].
 * Checks both the accessibility and developer-options animation scales.
 */
fun isReducedMotion(context: Context): Boolean {
    // Signal 1: ValueAnimator.areAnimatorsEnabled() (API 26+) — false when the
    // user has turned animations off via Accessibility "Remove animations" OR
    // set the developer-options animator duration scale to 0. (AccessibilityManager
    // has no public isAnimationEnabled property — that was the source of a build
    // break; areAnimatorsEnabled is the supported signal.)
    if (!android.animation.ValueAnimator.areAnimatorsEnabled()) return true

    // Signal 2 (belt-and-suspenders): read the duration scale directly in case a
    // ROM reports areAnimatorsEnabled()=true but pins the scale to 0.
    val scale = try {
        AndroidSettings.Global.getFloat(
            context.contentResolver,
            AndroidSettings.Global.ANIMATOR_DURATION_SCALE,
        )
    } catch (_: AndroidSettings.SettingNotFoundException) {
        1f  // setting absent → animations on
    }
    return scale == 0f
}

/**
 * Returns `true` when the user has toggled "Reduce motion" in the app Settings
 * (Display tab). This is the app-level calm/cinematic toggle and is OR-ed with
 * [rememberReducedMotion] in [motionDuration] so EITHER signal disables animation.
 */
@Composable
fun rememberUserMotionReduced(): Boolean {
    val context = LocalContext.current
    return remember(context) { Settings(context).motionReduced }
}

// ---------------------------------------------------------------------------

/**
 * Builds a dark Material3 ColorScheme from an [IdeColors] ramp.
 * Each palette gets its own scheme so primary/surface/error slots match the
 * palette accent and backgrounds without hardcoding the old DS-v2 constants.
 */
private fun darkColorSchemeFromRamp(c: IdeColors) = darkColorScheme(
    // ── Primary (accent) ──────────────────────────────────────────────────
    primary              = c.accent,
    onPrimary            = c.accentOn,
    primaryContainer     = c.accentDim,
    // onPrimaryContainer: accent text on the dim accent container.
    // LiquidTokens.accent2 is the ideal value, but IdeColors doesn't carry it —
    // use the lighter accent itself (readable on the dim container background).
    onPrimaryContainer   = c.accent,

    // ── Secondary (warning / amber) ───────────────────────────────────────
    secondary            = c.warning,
    onSecondary          = c.accentOn,
    secondaryContainer   = c.warningDim,
    onSecondaryContainer = c.warning,

    // ── Backgrounds / surfaces ────────────────────────────────────────────
    background           = c.bg,
    onBackground         = c.text,
    surface              = c.panel,
    onSurface            = c.text,
    surfaceVariant       = c.elevated,
    onSurfaceVariant     = c.dim,

    // Tonal surface containers — keep every tier inside the palette ramp.
    surfaceContainerLowest  = c.bg,
    surfaceContainerLow     = c.panel,
    surfaceContainer        = c.panel,
    surfaceContainerHigh    = c.elevated,
    surfaceContainerHighest = c.raised,

    // ── Outline / dividers ────────────────────────────────────────────────
    outline              = c.border,
    outlineVariant       = c.divider,

    // ── Error / destructive ───────────────────────────────────────────────
    error                = c.danger,
    onError              = Color.White,
    errorContainer       = c.dangerDim,
    onErrorContainer     = c.danger,

    // ── Scrim / inverse ───────────────────────────────────────────────────
    inverseSurface       = c.text,
    inverseOnSurface     = c.bg,
    inversePrimary       = c.accent,
    scrim                = Color.Black,
)

// The original Darcula scheme is kept as a named reference (backwards compat for
// any non-theme code that may import it directly; Theme.kt now builds per-palette).
private val DarculaColorScheme = darkColorSchemeFromRamp(DarkIdeColors)

// ---------------------------------------------------------------------------
// Light colour scheme — mirrors :root[data-theme="light"] in index.css.
// Dynamic color (Material You) is intentionally disabled to preserve the
// exact canonical palette regardless of the user's wallpaper.
// ---------------------------------------------------------------------------

/**
 * Builds a light Material3 ColorScheme from an [IdeColors] ramp.
 * Mirrors [darkColorSchemeFromRamp] for the light palette path.
 */
private fun lightColorSchemeFromRamp(c: IdeColors) = lightColorScheme(
    // ── Primary (accent) ──────────────────────────────────────────────────
    primary              = c.accent,
    onPrimary            = c.accentOn,
    primaryContainer     = c.accentDim,
    onPrimaryContainer   = c.accentPress,

    // ── Secondary (warning / amber) ───────────────────────────────────────
    secondary            = c.warning,
    onSecondary          = c.accentOn,
    secondaryContainer   = c.warningDim,
    onSecondaryContainer = c.warning,

    // ── Backgrounds / surfaces ────────────────────────────────────────────
    background           = c.bg,
    onBackground         = c.text,
    surface              = c.panel,
    onSurface            = c.text,
    surfaceVariant       = c.elevated,
    onSurfaceVariant     = c.dim,

    // Tonal surface containers.
    surfaceContainerLowest  = c.bg,
    surfaceContainerLow     = c.panel,
    surfaceContainer        = c.panel,
    surfaceContainerHigh    = c.elevated,
    surfaceContainerHighest = c.raised,

    // ── Outline / dividers ────────────────────────────────────────────────
    outline              = c.border,
    outlineVariant       = c.divider,

    // ── Error / destructive ───────────────────────────────────────────────
    error                = c.danger,
    onError              = Color.White,
    errorContainer       = c.dangerDim,
    onErrorContainer     = c.danger,

    // ── Scrim / inverse ───────────────────────────────────────────────────
    inverseSurface       = c.text,
    inverseOnSurface     = c.bg,
    inversePrimary       = c.accent,
    scrim                = Color.Black,
)

// Original LightColorScheme kept for backwards compat.
private val LightColorScheme = lightColorSchemeFromRamp(LightIdeColors)

/**
 * Reads the persisted [ThemeMode] from SharedPreferences (key "theme_mode",
 * default [ThemeMode.DARK] — dark-first for Graphite Mist per c48e spec).
 *
 * `remember(ctx)` so the read is stable across recompositions for the activity
 * lifetime; a Settings change recreates the activity, which re-reads it.
 */
@Composable
fun rememberThemeMode(): ThemeMode {
    val ctx = LocalContext.current
    return remember(ctx) { Settings(ctx).themeMode }
}

/**
 * Reads the persisted [Palette] from SharedPreferences (key "palette",
 * default [Palette.GRAPHITE_MIST] per c48e). Resolves unknown stored names
 * to [Palette.DEFAULT] defensively. `remember(ctx)` — stable for the activity
 * lifetime; a palette change recreates the activity.
 */
@Composable
fun rememberPalette(): Palette {
    val ctx = LocalContext.current
    return remember(ctx) {
        val name = Settings(ctx).paletteName
        Palette.entries.firstOrNull { it.name == name } ?: Palette.DEFAULT
    }
}

/**
 * Root theme for CopyPaste on Android.
 *
 * **Palette-driven** (c48e Liquid-Glass refresh): the active palette drives the
 * IdeColors ramp, LiquidTokens, AuroraDef, and Material3 ColorScheme.
 * Default palette is [Palette.GRAPHITE_MIST] (dark, cool grey).
 *
 * [themeMode] governs the light/dark axis independently of the palette:
 *   - [ThemeMode.DARK]   → use [palette]'s dark ramp (default)
 *   - [ThemeMode.LIGHT]  → use a light palette (falls back to [LightIdeColors]
 *                          when the chosen palette is a dark one)
 *   - [ThemeMode.SYSTEM] → follow OS ([isSystemInDarkTheme])
 *
 * Each palette carries [isDark] so a light-scheme palette selected via [palette]
 * overrides [themeMode] automatically (a light palette is always light).
 * Dynamic color (Material You) is disabled to preserve the exact canonical palette.
 */
@Composable
fun CopyPasteTheme(
    themeMode: ThemeMode = rememberThemeMode(),
    palette: Palette = rememberPalette(),
    skin: Skin = rememberSkin(),
    content: @Composable () -> Unit,
) {
    // Determine dark/light axis: palette.isDark takes priority when a palette
    // inherently belongs to a scheme (all current dark palettes are dark, light are
    // light). ThemeMode overrides only when palette could serve either axis —
    // but since the palette enum now carries isDark, we use it directly.
    // ThemeMode.SYSTEM still follows the OS when the user hasn't picked a specific
    // non-default palette (default is dark = Graphite Mist).
    // Theme axis (dark/light) is driven purely by the user's themeMode — the
    // palette is an independent CHROMA choice, so EVERY palette works in BOTH
    // dark and light (CopyPaste-s0uf parity). SYSTEM follows the OS.
    val darkTheme = when (themeMode) {
        ThemeMode.LIGHT  -> false
        ThemeMode.DARK   -> true
        ThemeMode.SYSTEM -> isSystemInDarkTheme()
    }

    // NEUTRALS from the theme axis, CHROMA (accent) from the palette.
    val resolvedPalette = palette
    val ideColors = paletteIdeColors(palette, darkTheme)
    val liquidTokens = paletteLiquidTokens(palette)

    // Build the Material3 ColorScheme from the resolved IdeColors ramp.
    val colorScheme = if (darkTheme) {
        darkColorSchemeFromRamp(ideColors)
    } else {
        lightColorSchemeFromRamp(ideColors)
    }

    val view = LocalView.current
    if (!view.isInEditMode) {
        SideEffect {
            val window = (view.context as Activity).window
            // Draw edge-to-edge: let our content extend behind the system bars and
            // the display cutout. The individual screens apply the corresponding
            // window insets (status-bar / cutout padding) so the header is never
            // clipped on notched / punch-hole phones. On API 35 this is enforced
            // by the platform regardless; we set it explicitly so API 26-34 behave
            // identically.
            WindowCompat.setDecorFitsSystemWindows(window, false)
            // Transparent status bar so the surface colour shows through.
            window.statusBarColor = Color.Transparent.toArgb()
            // Dark theme → light status-bar icons; light theme → dark icons.
            WindowCompat.getInsetsController(window, view).isAppearanceLightStatusBars = !darkTheme
            // Privacy: when the user disallows screenshots, set FLAG_SECURE so the
            // OS blocks screenshots, screen recording and the recents thumbnail for
            // every screen wrapped in CopyPasteTheme. Clipboard contents are
            // sensitive. The toggle recreate()s the activity so this re-applies.
            if (Settings(view.context).allowScreenshots) {
                window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
            } else {
                window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
            }
        }
    }

    // Provide all palette locals and the active skin alongside the Material colorScheme.
    // LocalSkin carries the structural/material token bundle — orthogonal to color.
    // A skin change triggers activity recreation (same lifecycle as palette), so
    // staticCompositionLocalOf in Skin.kt is correct — no incremental recomposition needed.
    CompositionLocalProvider(
        LocalPalette       provides resolvedPalette,
        LocalIdeColors     provides ideColors,
        LocalLiquidTokens  provides liquidTokens,
        LocalSkin          provides skin,
    ) {
        MaterialTheme(
            colorScheme = colorScheme,
            typography   = CopyPasteTypography,
            shapes       = CopyPasteShapes,
            content      = content,
        )
    }
}

// ---------------------------------------------------------------------------
// §8 Cinematic motion helpers (c48e)
//
// motionDuration(base) scales a [Motion] constant by the active palette's
// [LiquidTokens.motionScale], clamped to 0 when reduced-motion is active.
// Usage:
//   val dur = motionDuration(Motion.Base)   // = 180 * 1.3 = 234ms for Graphite Mist
// ---------------------------------------------------------------------------

/**
 * Returns the effective animation duration for [baseMs] (a [Motion] constant),
 * scaled by the active palette's [LiquidTokens.motionScale] and zeroed when
 * reduced motion is active.
 *
 * Three signals are checked; ANY of them independently zeroes the duration:
 *  1. OS accessibility / developer-options animation disable ([rememberReducedMotion])
 *  2. App-level "Reduce motion" toggle in Settings → Display ([rememberUserMotionReduced])
 *     — mirrors the web `data-motion="calm"` attribute set by the store's motionReduced key.
 *
 * Call from within a @Composable that has access to [LocalLiquidTokens].
 * The result is an Int suitable for [tween] durationMillis.
 */
@Composable
fun motionDuration(baseMs: Int): Int {
    val tokens = LocalLiquidTokens.current
    // Either OS-level or user-level reduced-motion preference disables animation.
    val reduced = rememberReducedMotion() || rememberUserMotionReduced()
    return if (reduced) 0 else (baseMs * tokens.motionScale).toInt()
}

// ---------------------------------------------------------------------------
// Switch styling moved to the bespoke IdeSwitch composable in Components.kt
// (PARITY-SPEC §7): the stock Material Switch could not express the canonical
// 34×18 dp track + 12 dp white thumb (both states) without a glow/state-layer
// halo, so a hand-drawn composable replaced the old ideSwitchColors() helper.
// ---------------------------------------------------------------------------
