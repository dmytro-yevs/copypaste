package com.copypaste.android.ui.theme

import androidx.compose.ui.graphics.Color
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * android-design-system "Explicit Material3 role table" + "CopyPasteTheme
 * Material3 role mapping" requirements: every consumed M3 role maps to the
 * canonical CpColors/AccentColor source, onError is pinned to black (not
 * white) in BOTH themes, and surfaceTint is Transparent.
 */
class CopyPasteThemeTest {

    @Test
    fun `container ladder resolves to the surface tokens not an M3 tonal default`() {
        val scheme = buildColorScheme(isDark = true, accent = AccentColor.INDIGO)
        assertEquals(DarkColors.bg, scheme.surfaceContainerLowest)
        assertEquals(DarkColors.panel, scheme.surfaceContainerLow)
        assertEquals(DarkColors.elevated, scheme.surfaceContainer)
        assertEquals(DarkColors.raised, scheme.surfaceContainerHigh)
        assertEquals(DarkColors.raised2, scheme.surfaceContainerHighest)
    }

    @Test
    fun `primary and onPrimary follow the active accent`() {
        val scheme = buildColorScheme(isDark = true, accent = AccentColor.ROSE)
        assertEquals(AccentColor.ROSE.dark, scheme.primary)
        assertEquals(AccentColor.ROSE.onDark, scheme.onPrimary)

        val lightScheme = buildColorScheme(isDark = false, accent = AccentColor.ROSE)
        assertEquals(AccentColor.ROSE.light, lightScheme.primary)
        assertEquals(AccentColor.ROSE.onLight, lightScheme.onPrimary)
    }

    @Test
    fun `onError is pinned to black in both themes and meets AA against err`() {
        val dark = buildColorScheme(isDark = true, accent = AccentColor.INDIGO)
        val light = buildColorScheme(isDark = false, accent = AccentColor.INDIGO)
        assertEquals(Color.Black, dark.onError)
        assertEquals(Color.Black, light.onError)
        assertEquals(DarkColors.err, dark.error)
        assertEquals(LightColors.err, light.error)

        val darkRatio = WcagContrast.ratio(dark.error, dark.onError)
        val lightRatio = WcagContrast.ratio(light.error, light.onError)
        // spec.md worked example: 6.32:1 dark, 4.80:1 light.
        assertEquals(6.32, darkRatio, 0.05)
        assertEquals(4.80, lightRatio, 0.05)
        assertTrue(darkRatio >= 4.5)
        assertTrue(lightRatio >= 4.5)
    }

    @Test
    fun `errorContainer is err at 12 percent and onErrorContainer is err`() {
        val scheme = buildColorScheme(isDark = true, accent = AccentColor.INDIGO)
        assertEquals(DarkColors.err.copy(alpha = 0.12f), scheme.errorContainer)
        assertEquals(DarkColors.err, scheme.onErrorContainer)
    }

    @Test
    fun `surfaceTint is transparent so elevation never washes surfaces`() {
        val scheme = buildColorScheme(isDark = true, accent = AccentColor.INDIGO)
        assertEquals(Color.Transparent, scheme.surfaceTint)
    }

    @Test
    fun `background surface outline and scrim roles map to CpColors`() {
        val scheme = buildColorScheme(isDark = false, accent = AccentColor.INDIGO)
        assertEquals(LightColors.bg, scheme.background)
        assertEquals(LightColors.text, scheme.onBackground)
        assertEquals(LightColors.panel, scheme.surface)
        assertEquals(LightColors.text, scheme.onSurface)
        assertEquals(LightColors.elevated, scheme.surfaceVariant)
        assertEquals(LightColors.dim, scheme.onSurfaceVariant)
        assertEquals(LightColors.border, scheme.outline)
        assertEquals(LightColors.divider, scheme.outlineVariant)
        assertEquals(LightColors.scrim, scheme.scrim)
    }

    @Test
    fun `system bar appearance is light icons in light theme and dark icons in dark theme`() {
        assertEquals(false, systemBarsAreLight(isDark = true))
        assertEquals(true, systemBarsAreLight(isDark = false))
    }
}
