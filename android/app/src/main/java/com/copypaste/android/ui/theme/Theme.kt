package com.copypaste.android.ui.theme

import android.app.Activity
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
// CopyPaste theme — always dark, matching the macOS JetBrains "New UI" /
// Darcula palette (v0.5.3) used in crates/copypaste-ui/tailwind.config.js.
//
// Dynamic color (Material You) is intentionally disabled: it would override
// the precise Darcula palette we need to match the desktop app.
// ---------------------------------------------------------------------------

private val DarculaColorScheme = darkColorScheme(
    // ── Primary (accent blue) ─────────────────────────────────────────────
    primary              = DarkPrimary,            // #3592ff — action blue
    onPrimary            = DarkOnPrimary,           // white on blue
    primaryContainer     = DarkPrimaryContainer,    // deep blue tint
    onPrimaryContainer   = DarkOnPrimaryContainer,

    // ── Secondary (amber / warning) ───────────────────────────────────────
    secondary            = DarkSecondary,
    onSecondary          = DarkOnSecondary,
    secondaryContainer   = DarkSecondaryContainer,
    onSecondaryContainer = DarkOnSecondaryContainer,

    // ── Backgrounds / surfaces ────────────────────────────────────────────
    background           = IdeBg,        // #16171a — outermost bg (darker)
    onBackground         = IdeText,      // #dfe1e5
    surface              = IdePanel,     // #1e2024 — primary surface
    onSurface            = IdeText,
    surfaceVariant       = IdeElevated,  // #26282d — cards, chips, inputs
    onSurfaceVariant     = IdeDim,       // #9da0a8

    // Tonal surface containers — keep every elevation tier inside the Darcula
    // grey ramp instead of Material3's default purple-tinted auto-elevation.
    surfaceContainerLowest  = IdeBg,       // #16171a
    surfaceContainerLow     = IdePanel,    // #1e2024
    surfaceContainer        = IdePanel,    // #1e2024 — bottom nav / app bar
    surfaceContainerHigh    = IdeElevated, // #26282d — cards
    surfaceContainerHighest = IdeRaised,   // #2d2f34 — pressed / raised

    // ── Outline / dividers ────────────────────────────────────────────────
    outline              = IdeBorder,    // #383b42
    outlineVariant       = IdeDivider,   // #2e3035

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
 * macOS desktop app which is permanently dark (JetBrains Darcula palette).
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
