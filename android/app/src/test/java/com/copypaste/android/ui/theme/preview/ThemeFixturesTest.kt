package com.copypaste.android.ui.theme.preview

import com.copypaste.android.ui.theme.AccentColor
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * android-visual-regression "Representative Fixture Matrix Without Full
 * Cross-Product": the catalog's default axis covers dark+light, >=2 accents,
 * EN+UK — and stays far below the full 3-theme x 6-accent x 2-translucency x
 * 2-locale = 72-combination cross-product this requirement forbids rendering.
 */
class ThemeFixturesTest {

    @Test
    fun `representative set covers both themes`() {
        val themes = ThemeFixtures.representative.map { it.isDark }.toSet()
        assertEquals(setOf(true, false), themes)
    }

    @Test
    fun `representative set covers at least two distinct accents`() {
        val accents = ThemeFixtures.representative.map { it.accent }.toSet()
        assertTrue("expected >= 2 accents, got $accents", accents.size >= 2)
    }

    @Test
    fun `representative set covers EN and UK`() {
        val locales = ThemeFixtures.representative.map { it.locale }.toSet()
        assertEquals(setOf("en", "uk"), locales)
    }

    @Test
    fun `representative set is far smaller than the full cross-product`() {
        // 3 themes (dark/light/system) x 6 accents x 2 translucency x 2 locales = 72;
        // the representative set must stay well under that.
        assertTrue(ThemeFixtures.representative.size < 72 / 4)
    }

    @Test
    fun `fixture label is stable and filename-safe`() {
        val fixture = ThemeFixtures.DarkIndigo
        assertEquals("dark_indigo_en", fixture.label)
        assertTrue(fixture.label.none { it.isWhitespace() })
    }

    @Test
    fun `provider values match the representative set`() {
        val provided = ThemeFixtureProvider().values.toList()
        assertEquals(ThemeFixtures.representative, provided)
    }

    @Test
    fun `every accent enum value is reachable from AccentColor fromName`() {
        for (accent in AccentColor.entries) {
            assertEquals(accent, AccentColor.fromName(accent.name))
        }
    }
}
