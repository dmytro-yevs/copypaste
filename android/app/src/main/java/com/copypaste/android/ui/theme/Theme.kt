package com.copypaste.android.ui.theme

import android.app.Activity
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat

// ---------------------------------------------------------------------------
// CopyPaste theme — always dark, matching the macOS JetBrains "New UI" /
// Darcula palette used in crates/copypaste-ui/tailwind.config.js.
//
// Dynamic color (Material You) is intentionally disabled: it would override
// the precise Darcula palette we need to match the desktop app.
// ---------------------------------------------------------------------------

private val DarculaColorScheme = darkColorScheme(
    // ── Primary (accent blue) ─────────────────────────────────────────────
    primary              = DarkPrimary,            // #3574f0 — action blue
    onPrimary            = DarkOnPrimary,           // white on blue
    primaryContainer     = DarkPrimaryContainer,    // deep blue tint
    onPrimaryContainer   = DarkOnPrimaryContainer,

    // ── Secondary (amber / warning) ───────────────────────────────────────
    secondary            = DarkSecondary,
    onSecondary          = DarkOnSecondary,
    secondaryContainer   = DarkSecondaryContainer,
    onSecondaryContainer = DarkOnSecondaryContainer,

    // ── Backgrounds / surfaces ────────────────────────────────────────────
    background           = IdeBg,       // #1e1f22 — outermost bg
    onBackground         = IdeText,     // #dfe1e5
    surface              = IdePanel,    // #2b2d30 — primary surface
    onSurface            = IdeText,
    surfaceVariant       = IdeElevated, // #313438 — cards, chips
    onSurfaceVariant     = IdeDim,      // #9da0a8

    // ── Outline / dividers ────────────────────────────────────────────────
    outline              = IdeBorder,   // #393b40
    outlineVariant       = IdeDivider,  // #43454a

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
            // Match status bar to our root background so the UI looks edge-to-edge.
            window.statusBarColor = IdeBg.toArgb()
            // Dark status bar = light icons (correct for dark background).
            WindowCompat.getInsetsController(window, view).isAppearanceLightStatusBars = false
        }
    }

    MaterialTheme(
        colorScheme = DarculaColorScheme,
        typography   = CopyPasteTypography,
        content      = content,
    )
}
