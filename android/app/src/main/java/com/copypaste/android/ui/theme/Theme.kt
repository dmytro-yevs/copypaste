package com.copypaste.android.ui.theme

import android.app.Activity
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat

// ---------------------------------------------------------------------------
// CopyPaste theme — always dark, matching the Design System v2 "Quiet
// Precision" canonical palette defined in DESIGN-SYSTEM-v2.md §0/§3 and
// mirrored in crates/copypaste-ui/tailwind.config.js.
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

/**
 * Root theme for CopyPaste on Android.
 *
 * Always renders in dark mode regardless of system setting, matching the
 * macOS desktop app which is permanently dark (Design System v2 palette).
 * Dynamic color (Material You) is disabled to preserve the exact palette.
 */
@Composable
fun CopyPasteTheme(
    // Parameter kept for call-site compatibility but ignored — always dark.
    @Suppress("UNUSED_PARAMETER") darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
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
            // Transparent status bar so our dark background shows through the
            // cutout / status-bar strip.
            window.statusBarColor = Color.Transparent.toArgb()
            // Dark background → light status-bar icons.
            WindowCompat.getInsetsController(window, view).isAppearanceLightStatusBars = false
        }
    }

    MaterialTheme(
        colorScheme = DarculaColorScheme,
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
