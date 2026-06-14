package com.copypaste.android.ui.theme

import android.app.Activity
import android.content.Context
import android.provider.Settings as AndroidSettings
import android.view.accessibility.AccessibilityManager
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat

// ---------------------------------------------------------------------------
// CopyPaste theme — dark by default, matching the Design System v2 "Quiet
// Precision" canonical palette defined in DESIGN-SYSTEM-v2.md §0/§3 and
// mirrored in crates/copypaste-ui/tailwind.config.js.
//
// A light colour scheme (LightColorScheme) is also provided; pass
// darkTheme=false to CopyPasteTheme() to use it.
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

// ---------------------------------------------------------------------------

private val DarculaColorScheme = darkColorScheme(
    // ── Primary (accent blue) ─────────────────────────────────────────────
    primary              = DarkPrimary,            // #3D8BFF — canonical accent
    onPrimary            = DarkOnPrimary,           // white on blue
    primaryContainer     = DarkPrimaryContainer,    // deep blue tint
    onPrimaryContainer   = DarkOnPrimaryContainer,

    // ── Secondary (amber / warning) ───────────────────────────────────────
    secondary            = DarkSecondary,
    onSecondary          = DarkOnSecondary,
    secondaryContainer   = DarkSecondaryContainer,
    onSecondaryContainer = DarkOnSecondaryContainer,

    // ── Backgrounds / surfaces ────────────────────────────────────────────
    background           = IdeBg,        // #13141A — §0 canonical bg
    onBackground         = IdeText,      // #E8EAED — §0 canonical text
    surface              = IdePanel,     // #1B1C22 — §0 canonical panel
    onSurface            = IdeText,
    surfaceVariant       = IdeElevated,  // #23252D — §0 canonical elevated
    onSurfaceVariant     = IdeDim,       // #9DA0A8

    // Tonal surface containers — keep every elevation tier inside the canonical
    // grey ramp instead of Material3's default purple-tinted auto-elevation.
    surfaceContainerLowest  = IdeBg,       // #13141A
    surfaceContainerLow     = IdePanel,    // #1B1C22
    surfaceContainer        = IdePanel,    // #1B1C22 — bottom nav / app bar
    surfaceContainerHigh    = IdeElevated, // #23252D — cards
    surfaceContainerHighest = IdeRaised,   // #2D2F34 — pressed / raised

    // ── Outline / dividers ────────────────────────────────────────────────
    outline              = IdeBorder,    // #383B42
    outlineVariant       = IdeDivider,   // #2E3035

    // ── Error / destructive ───────────────────────────────────────────────
    error                = IdeDanger,
    onError              = Color.White,
    errorContainer       = IdeErrorContainer,
    onErrorContainer     = IdeOnErrorContainer,

    // ── Scrim / inverse (kept at safe defaults) ───────────────────────────
    inverseSurface       = IdeText,
    inverseOnSurface     = IdeBg,
    inversePrimary       = IdeAccent,
    scrim                = Color.Black,
)

// ---------------------------------------------------------------------------
// Light colour scheme — mirrors :root[data-theme="light"] in index.css.
// Dynamic color (Material You) is intentionally disabled to preserve the
// exact canonical palette regardless of the user's wallpaper.
// ---------------------------------------------------------------------------

private val LightColorScheme = lightColorScheme(
    // ── Primary (accent blue — darkened for light surfaces) ───────────────
    primary              = LightPrimary,            // #1A5FCC — 5.2:1 on elevated
    onPrimary            = LightOnPrimary,           // white on blue
    primaryContainer     = LightPrimaryContainer,    // light blue tint
    onPrimaryContainer   = LightOnPrimaryContainer,

    // ── Secondary (amber / warning) ───────────────────────────────────────
    secondary            = LightSecondary,
    onSecondary          = LightOnSecondary,
    secondaryContainer   = LightSecondaryContainer,
    onSecondaryContainer = LightOnSecondaryContainer,

    // ── Backgrounds / surfaces ────────────────────────────────────────────
    background           = LightBg,        // #ECEEF2 — lightest layer
    onBackground         = LightText,      // #1A1C20 — 13.8:1 on bg
    surface              = LightPanel,     // #F5F6F8 — primary surface
    onSurface            = LightText,
    surfaceVariant       = LightElevated,  // #EEF0F4 — elevated
    onSurfaceVariant     = LightDim,       // #4B505A — 6.2:1 on panel

    // Tonal surface containers — keep every tier inside the canonical light ramp.
    surfaceContainerLowest  = LightBg,       // #ECEEF2
    surfaceContainerLow     = LightPanel,    // #F5F6F8
    surfaceContainer        = LightPanel,    // #F5F6F8 — bottom nav / app bar
    surfaceContainerHigh    = LightElevated, // #EEF0F4 — cards
    surfaceContainerHighest = LightRaised,   // #E4E6EB — pressed / raised

    // ── Outline / dividers ────────────────────────────────────────────────
    outline              = LightBorder,    // #C8CAD0
    outlineVariant       = LightDivider,   // #D8DAE0

    // ── Error / destructive ───────────────────────────────────────────────
    error                = LightDanger,
    onError              = Color.White,
    errorContainer       = LightErrorContainer,
    onErrorContainer     = LightOnErrorContainer,

    // ── Scrim / inverse ───────────────────────────────────────────────────
    inverseSurface       = LightText,
    inverseOnSurface     = LightBg,
    inversePrimary       = IdeAccent,
    scrim                = Color.Black,
)

/**
 * Root theme for CopyPaste on Android.
 *
 * Renders in dark mode by default (matching the Design System v2 palette) but
 * honors the [darkTheme] parameter: pass `false` to use the WCAG-AA light
 * colour scheme. Dynamic color (Material You) is disabled to preserve the
 * exact canonical palette.
 */
@Composable
fun CopyPasteTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    val colorScheme = if (darkTheme) DarculaColorScheme else LightColorScheme
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
            // Light theme → dark status-bar icons; dark theme → light icons.
            WindowCompat.getInsetsController(window, view).isAppearanceLightStatusBars = !darkTheme
        }
    }

    MaterialTheme(
        colorScheme = colorScheme,
        typography   = CopyPasteTypography,
        shapes       = CopyPasteShapes,
        content      = content,
    )
}

// ---------------------------------------------------------------------------
// Shared component color overrides — used at call sites to keep all screens
// consistent without repeating the same color arguments everywhere.
// ---------------------------------------------------------------------------

/**
 * IDE-styled Switch colors: accent thumb when checked, ide-elevated track
 * with ide-border outline when unchecked. Matches the macOS Toggle component.
 */
@Composable
fun ideSwitchColors() = SwitchDefaults.colors(
    checkedThumbColor        = Color.White,
    checkedTrackColor        = IdeAccent,
    checkedBorderColor       = IdeAccent,
    uncheckedThumbColor      = IdeDim,
    uncheckedTrackColor      = IdeElevated,
    uncheckedBorderColor     = IdeBorder,
    disabledCheckedThumbColor    = Color.White.copy(alpha = 0.38f),
    disabledCheckedTrackColor    = IdeAccent.copy(alpha = 0.38f),
    disabledUncheckedThumbColor  = IdeDim.copy(alpha = 0.38f),
    disabledUncheckedTrackColor  = IdeElevated.copy(alpha = 0.38f),
)
