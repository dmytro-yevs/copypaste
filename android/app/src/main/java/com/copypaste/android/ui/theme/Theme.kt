package com.copypaste.android.ui.theme

import android.app.Activity
import android.content.Context
import android.provider.Settings as AndroidSettings
import android.view.WindowManager
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

// ---------------------------------------------------------------------------
// CopyPaste theme — two axes only (STYLEGUIDE §11): isDark × accent.
//
// There are no palettes, skins, density modes, contrast modes or motion modes.
// The theme axis is driven by the persisted [ThemeMode] (System / Light / Dark);
// the accent axis by the persisted [AccentColor]. `CopyPasteTheme` provides
// `LocalCpColors`, `LocalAccent` (the new cross-platform contract) and the
// legacy `LocalIdeColors` adapter alongside a Material3 scheme.
//
// Dynamic color (Material You) is intentionally disabled: it would override the
// precise canonical palette we need to match the desktop app.
// ---------------------------------------------------------------------------

// ── §6 Motion tokens (mirrors index.css; reduced collapses all to 0ms) ──────
object Motion {
    const val Instant = 90
    const val Fast    = 130
    const val Base    = 180
    const val Slow    = 240
}

/** Active two-axis tokens, provided by [CopyPasteTheme]. */
val LocalCpColors = staticCompositionLocalOf { DarkColors }

/** Active accent, provided by [CopyPasteTheme]. */
val LocalAccent = staticCompositionLocalOf { AccentColor.DEFAULT }

/**
 * Legacy adapter bundle (derived from [LocalCpColors] + [LocalAccent]) read by
 * existing screens as `LocalIdeColors.current.<token>`. Defaults to the dark
 * adapter so any stray reader outside [CopyPasteTheme] is still defined.
 */
val LocalIdeColors = staticCompositionLocalOf { DarkIdeColors }

/** §6 out-expo easing — matches CSS cubic-bezier(.16,1,.3,1). */
val EaseOutExpo = CubicBezierEasing(0.16f, 1.0f, 0.3f, 1.0f)

/** §6 standard easing — matches CSS cubic-bezier(.2,.8,.2,1). */
val EaseStandard = CubicBezierEasing(0.20f, 0.80f, 0.2f, 1.0f)

/** §6 ease-in — matches CSS cubic-bezier(.4,0,1,1). */
val EaseIn = CubicBezierEasing(0.40f, 0.0f, 1.0f, 1.0f)

// ---------------------------------------------------------------------------
// §6 Reduced-motion gate — mirrors the web prefers-reduced-motion media query.
// Honored automatically; it is NOT a user setting (STYLEGUIDE §2).
// ---------------------------------------------------------------------------

/** Returns `true` when the user has requested reduced motion via the platform. */
@Composable
fun rememberReducedMotion(): Boolean {
    val context = LocalContext.current
    return remember(context) { isReducedMotion(context) }
}

/** Non-composable helper — checks both the accessibility + developer-options scales. */
fun isReducedMotion(context: Context): Boolean {
    if (!android.animation.ValueAnimator.areAnimatorsEnabled()) return true
    val scale = try {
        AndroidSettings.Global.getFloat(
            context.contentResolver,
            AndroidSettings.Global.ANIMATOR_DURATION_SCALE,
        )
    } catch (_: AndroidSettings.SettingNotFoundException) {
        1f
    }
    return scale == 0f
}

/**
 * Reads the persisted [ThemeMode] (key "theme_mode", default [ThemeMode.DEFAULT]).
 * `remember(ctx)` so the read is stable for the activity lifetime; a Settings
 * change recreates the activity, which re-reads it.
 */
@Composable
fun rememberThemeMode(): ThemeMode {
    val ctx = LocalContext.current
    return remember(ctx) { Settings(ctx).themeMode }
}

/** Reads the persisted [AccentColor] (key "accent", default [AccentColor.DEFAULT]). */
@Composable
fun rememberAccent(): AccentColor {
    val ctx = LocalContext.current
    return remember(ctx) { Settings(ctx).accent }
}

/**
 * Root theme — collapses to isDark × accent (STYLEGUIDE §11).
 *
 * [themeMode] governs the dark/light axis; [accent] the chroma axis. Both are
 * read from SharedPreferences by default so every screen follows the user's
 * choice without per-call-site wiring.
 */
@Composable
fun CopyPasteTheme(
    themeMode: ThemeMode = rememberThemeMode(),
    accent: AccentColor = rememberAccent(),
    content: @Composable () -> Unit,
) {
    val isDark = when (themeMode) {
        ThemeMode.LIGHT  -> false
        ThemeMode.DARK   -> true
        ThemeMode.SYSTEM -> isSystemInDarkTheme()
    }

    val cp = if (isDark) DarkColors else LightColors
    val ide = cpToIde(cp, accent, isDark)

    val scheme = (if (isDark) darkColorScheme() else lightColorScheme()).copy(
        primary                 = accent.base(isDark),
        onPrimary               = accent.on(isDark),
        primaryContainer        = ide.accentDim,
        onPrimaryContainer      = accent.base(isDark),
        secondary               = cp.warn,
        onSecondary             = accent.on(isDark),
        secondaryContainer      = ide.warningDim,
        onSecondaryContainer    = cp.warn,
        background              = cp.bg,
        onBackground            = cp.text,
        surface                 = cp.panel,
        onSurface               = cp.text,
        surfaceVariant          = cp.elevated,
        onSurfaceVariant        = cp.dim,
        surfaceContainerLowest  = cp.bg,
        surfaceContainerLow     = cp.panel,
        surfaceContainer        = cp.panel,
        surfaceContainerHigh    = cp.elevated,
        surfaceContainerHighest = cp.raised,
        outline                 = cp.border,
        outlineVariant          = cp.divider,
        error                   = cp.err,
        onError                 = Color.White,
        errorContainer          = ide.dangerDim,
        onErrorContainer        = cp.err,
        inverseSurface          = cp.text,
        inverseOnSurface        = cp.bg,
        inversePrimary          = accent.base(isDark),
        scrim                   = Color.Black,
    )

    val view = LocalView.current
    if (!view.isInEditMode) {
        SideEffect {
            val window = (view.context as Activity).window
            WindowCompat.setDecorFitsSystemWindows(window, false)
            window.statusBarColor = Color.Transparent.toArgb()
            WindowCompat.getInsetsController(window, view).isAppearanceLightStatusBars = !isDark
            // Privacy: honor the screenshot policy for every themed screen.
            if (Settings(view.context).allowScreenshots) {
                window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
            } else {
                window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
            }
        }
    }

    CompositionLocalProvider(
        LocalCpColors  provides cp,
        LocalAccent    provides accent,
        LocalIdeColors provides ide,
    ) {
        MaterialTheme(
            colorScheme = scheme,
            typography  = CopyPasteTypography,
            shapes      = CopyPasteShapes,
            content     = content,
        )
    }
}

/**
 * Returns the effective animation duration for [baseMs], zeroed when reduced
 * motion is active (STYLEGUIDE §6). No palette motion-scale: motion is a fixed,
 * quiet language with a single reduced-motion gate.
 */
@Composable
fun motionDuration(baseMs: Int): Int = if (rememberReducedMotion()) 0 else baseMs
