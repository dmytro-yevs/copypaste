package com.copypaste.android.ui.theme.preview

import androidx.compose.runtime.Composable
import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import com.copypaste.android.ui.theme.AccentColor
import com.copypaste.android.ui.theme.CopyPasteTheme

// ---------------------------------------------------------------------------
// Central preview catalog scaffolding (task 2.4, android-visual-regression
// "Central Preview Catalog" requirement): a single source of themed fixtures
// built on PreviewParameterProvider, so new composables register fixtures
// here instead of declaring bespoke duplicated @Preview functions. Screen
// slices (S4+) extend this catalog with their own state fixtures (loading/
// empty/error/masked/…); S2 only establishes the mechanism + the
// representative theme/accent/locale axis every screen fixture composes with.
//
// Per android-visual-regression "Representative Fixture Matrix Without Full
// Cross-Product": THEME_FIXTURES below is the representative set (dark+light,
// >=2 accents, EN+UK) — never the full 3-theme x 6-accent x 2-translucency x
// 2-locale cross-product.
// ---------------------------------------------------------------------------

/** One themed rendering context: dark/light x accent x locale. */
data class ThemeFixture(
    val isDark: Boolean,
    val accent: AccentColor,
    val locale: String = "en",
    val translucency: Boolean = true,
) {
    /** Stable name for golden filenames / test-method-name suffixes. */
    val label: String
        get() = "${if (isDark) "dark" else "light"}_${accent.name.lowercase()}_$locale"
}

/**
 * The representative theme/accent/locale axis (android-visual-regression
 * "dark and light themes, at least two accents ... English and Ukrainian for
 * text-heavy screens"). Individual screen fixtures pick the subset relevant
 * to their own coverage instead of re-deriving this axis.
 */
object ThemeFixtures {
    val DarkIndigo = ThemeFixture(isDark = true, accent = AccentColor.INDIGO, locale = "en")
    val LightIndigo = ThemeFixture(isDark = false, accent = AccentColor.INDIGO, locale = "en")
    val DarkTeal = ThemeFixture(isDark = true, accent = AccentColor.TEAL, locale = "en")
    val DarkIndigoUk = ThemeFixture(isDark = true, accent = AccentColor.INDIGO, locale = "uk")

    /** The default representative set consumed by [ThemeFixtureProvider]. */
    val representative: List<ThemeFixture> = listOf(DarkIndigo, LightIndigo, DarkTeal, DarkIndigoUk)
}

/** `@PreviewParameter`-compatible provider over [ThemeFixtures.representative]. */
class ThemeFixtureProvider : PreviewParameterProvider<ThemeFixture> {
    override val values: Sequence<ThemeFixture> = ThemeFixtures.representative.asSequence()
}

/**
 * Wraps [content] in [CopyPasteTheme] resolved from [fixture] — the single
 * entry point every catalog fixture/golden/preview uses instead of
 * hand-rolling `CopyPasteTheme(...)` at each call site.
 */
@Composable
fun CpPreviewScaffold(fixture: ThemeFixture, content: @Composable () -> Unit) {
    CopyPasteTheme(isDark = fixture.isDark, accent = fixture.accent, translucency = fixture.translucency) {
        content()
    }
}
