package com.copypaste.android.ui.theme

import android.app.Activity
import android.view.WindowManager
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsControllerCompat
import com.copypaste.android.Settings

/**
 * Neutral window chrome — keeps the two FUNCTIONAL SideEffects every themed
 * screen needs:
 *   1. Edge-to-edge: WindowCompat.setDecorFitsSystemWindows(window, false)
 *   2. FLAG_SECURE screenshot-privacy policy driven by Settings.allowScreenshots
 *
 * No palette, no accent, no Typography override, no Shapes override.
 * Wraps content in a plain MaterialTheme so every screen gets M3 defaults.
 *
 * SECURITY: MainActivity / HistoryActivity / DevicesActivity / LogViewerActivity
 * removed their local FLAG_SECURE calls and rely on this SideEffect for
 * screenshot protection of sensitive clipboard content. Do NOT remove it.
 */
@Composable
fun SecureWindowChrome(content: @Composable () -> Unit) {
    val view = LocalView.current
    if (!view.isInEditMode) {
        SideEffect {
            val window = (view.context as Activity).window
            // Edge-to-edge: let Compose manage insets instead of the window.
            WindowCompat.setDecorFitsSystemWindows(window, false)
            // Privacy: honor the per-user screenshot policy for every themed screen.
            if (Settings(view.context).allowScreenshots) {
                window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
            } else {
                window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
            }
        }
    }
    MaterialTheme(content = content)
}

// ---------------------------------------------------------------------------
// CopyPasteTheme — the two-axis design system root (design.md D2, S1.2).
//
// `CopyPasteTheme(isDark, accent, translucency, content)` provides
// LocalCpColors/LocalAccent AND a fully-explicit M3 ColorScheme (every
// consumed role mapped — android-design-system "Explicit Material3 role table"
// — so no default tonal/purple role can leak through a Material component).
//
// This is layered ON TOP OF (not a replacement for) SecureWindowChrome: call
// sites wrap `SecureWindowChrome { CopyPasteTheme(...) { content() } }` so the
// two preserved security SideEffects above stay verbatim, and the new
// system-bar-appearance SideEffect below is scoped to CopyPasteTheme only —
// it is intentionally NOT added to SecureWindowChrome (D16).
// ---------------------------------------------------------------------------

/** Active two-axis tokens, provided by [CopyPasteTheme]. */
val LocalCpColors = staticCompositionLocalOf { DarkColors }

/** Active accent, provided by [CopyPasteTheme]. */
val LocalAccent = staticCompositionLocalOf { AccentColor.DEFAULT }

/**
 * Resolved-theme → status/nav-bar icon appearance (D16 part 3): light icons
 * (dark bars) in dark theme, dark icons (light bars) in light theme. Pure
 * function so the mapping is unit-testable without a Window/Robolectric —
 * the actual `WindowInsetsControllerCompat` call is a thin SideEffect below.
 */
internal fun systemBarsAreLight(isDark: Boolean): Boolean = !isDark

/**
 * Builds the full explicit M3 [ColorScheme] for [isDark]/[accent] per the
 * android-design-system role table: primary/onPrimary from the accent,
 * background/surface from bg/panel, the surfaceContainer ladder from
 * elevated/raised/raised2, error/onError pinned to `#000000` in BOTH themes
 * (NOT white — spec.md's worked WCAG example: white measures 3.32:1 on dark
 * err and 4.38:1 on light err, both failing AA; black measures 6.32:1 / 4.80:1),
 * and `surfaceTint = Transparent` so the brand accent — not Material You — is
 * authoritative.
 */
internal fun buildColorScheme(isDark: Boolean, accent: AccentColor): ColorScheme {
    val cp = if (isDark) DarkColors else LightColors
    return buildColorScheme(cp, primary = accent.base(isDark), onPrimary = accent.on(isDark), isDark = isDark)
}

/**
 * Same role mapping as the [buildColorScheme] overload above, but taking
 * already-resolved [cp]/[primary]/[onPrimary] — used by the animated
 * crossfade path ([CopyPasteTheme]) so the SAME role-mapping formulas apply
 * whether the inputs are static tokens or in-flight animated `Color`s.
 */
internal fun buildColorScheme(cp: CpColors, primary: Color, onPrimary: Color, isDark: Boolean): ColorScheme {
    val base = if (isDark) darkColorScheme() else lightColorScheme()
    return base.copy(
        primary = primary,
        onPrimary = onPrimary,
        background = cp.bg,
        onBackground = cp.text,
        surface = cp.panel,
        onSurface = cp.text,
        surfaceVariant = cp.elevated,
        onSurfaceVariant = cp.dim,
        surfaceContainerLowest = cp.bg,
        surfaceContainerLow = cp.panel,
        surfaceContainer = cp.elevated,
        surfaceContainerHigh = cp.raised,
        surfaceContainerHighest = cp.raised2,
        outline = cp.border,
        outlineVariant = cp.divider,
        error = cp.err,
        // Pinned #000000 in both themes — NOT white. See kdoc above.
        onError = Color.Black,
        errorContainer = cp.err.copy(alpha = 0.12f),
        onErrorContainer = cp.err,
        scrim = cp.scrim,
        surfaceTint = Color.Transparent,
    )
}

/**
 * Root theme — two axes (STYLEGUIDE §11): [isDark] × [accent], plus
 * [translucency] (consumed by the D7 blur policy at the surfaces that render
 * chrome — S1 only carries the flag through, see [BlurMode]/[resolveBlurMode]).
 *
 * Callers supply the already-resolved [isDark] (System/Dark/Light resolution
 * to a boolean is a Settings-layer concern owned by S3); [accent] defaults to
 * [AccentColor.DEFAULT] and [translucency] defaults to on (D4 default). A
 * theme or accent change crossfades every token over `--dur-theme` (300ms,
 * task 1.6) instead of snapping — see [animateCpColorsCrossfade]/
 * [animateAccentCrossfade] — collapsed to an instant swap under reduced motion.
 */
@Composable
fun CopyPasteTheme(
    isDark: Boolean,
    accent: AccentColor = AccentColor.DEFAULT,
    translucency: Boolean = true,
    content: @Composable () -> Unit,
) {
    val reduced = rememberCpMotionReduced()
    val cp = animateCpColorsCrossfade(isDark, reduced)
    val (primary, onPrimary) = animateAccentCrossfade(isDark, accent, reduced)
    val scheme = buildColorScheme(cp, primary, onPrimary, isDark)

    val view = LocalView.current
    if (!view.isInEditMode) {
        SideEffect {
            val window = (view.context as Activity).window
            val controller = WindowInsetsControllerCompat(window, view)
            val light = systemBarsAreLight(isDark)
            controller.isAppearanceLightStatusBars = light
            controller.isAppearanceLightNavigationBars = light
        }
    }

    CompositionLocalProvider(
        LocalCpColors provides cp,
        LocalAccent provides accent,
        LocalTranslucencyEnabled provides translucency,
    ) {
        MaterialTheme(
            colorScheme = scheme,
            typography = CopyPasteTypography,
            shapes = CopyPasteShapes,
            content = content,
        )
    }
}
