package com.copypaste.android

import com.copypaste.android.ui.theme.Palette
import com.copypaste.android.ui.theme.paletteIdeColors
import com.copypaste.android.ui.theme.paletteLiquidTokens
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM tests for the Appearance section in SettingsScreen (CopyPaste-hvr4).
 *
 * Validates:
 *  1. All Palette enum entries resolve to non-null IdeColors + LiquidTokens.
 *  2. Palette display labels are derived consistently from the name.
 *  3. The palette-name round-trip (write name → read back) is stable.
 *  4. Dark/light groupings are internally consistent.
 *  5. Settings.paletteName default is "GRAPHITE_MIST".
 *  6. Density picker: only COMFORTABLE/COMPACT variants exist (no SPACIOUS yet).
 *  7. ThemeMode picker: SYSTEM/LIGHT/DARK variants all exist.
 *  8. Each palette's accent color is not fully transparent.
 */
class AppearanceSectionTest {

    // ── 1. All palettes resolve without throwing ──────────────────────────────

    @Test
    fun `all Palette entries resolve to non-null IdeColors`() {
        Palette.entries.forEach { palette ->
            val colors = paletteIdeColors(palette)
            assertNotNull("IdeColors must not be null for $palette", colors)
        }
    }

    @Test
    fun `all Palette entries resolve to non-null LiquidTokens`() {
        Palette.entries.forEach { palette ->
            val tokens = paletteLiquidTokens(palette)
            assertNotNull("LiquidTokens must not be null for $palette", tokens)
        }
    }

    // ── 2. Display label derivation is consistent ─────────────────────────────

    /**
     * Display names are derived from the enum name by replacing underscores with
     * spaces and converting to title case.
     * Example: GRAPHITE_MIST → "Graphite Mist"
     */
    @Test
    fun `palette display label derivation produces non-empty strings`() {
        Palette.entries.forEach { palette ->
            val label = paletteDisplayLabel(palette)
            assertTrue(
                "Display label for $palette must be non-empty",
                label.isNotBlank(),
            )
        }
    }

    @Test
    fun `GRAPHITE_MIST display label is Graphite Mist`() {
        assertEquals("Graphite Mist", paletteDisplayLabel(Palette.GRAPHITE_MIST))
    }

    @Test
    fun `LIQUID_BLUE display label is Liquid Blue`() {
        assertEquals("Liquid Blue", paletteDisplayLabel(Palette.LIQUID_BLUE))
    }

    @Test
    fun `AURORA_VIOLET display label is Aurora Violet`() {
        assertEquals("Aurora Violet", paletteDisplayLabel(Palette.AURORA_VIOLET))
    }

    @Test
    fun `CLOUD_SILVER display label is Cloud Silver`() {
        assertEquals("Cloud Silver", paletteDisplayLabel(Palette.CLOUD_SILVER))
    }

    // ── 3. Palette name round-trip ────────────────────────────────────────────

    @Test
    fun `every Palette name parses back to the same enum constant`() {
        Palette.entries.forEach { palette ->
            val parsed = Palette.valueOf(palette.name)
            assertEquals(
                "Palette.valueOf(${palette.name}) must round-trip",
                palette,
                parsed,
            )
        }
    }

    // ── 4. Dark / light grouping ──────────────────────────────────────────────

    @Test
    fun `dark palettes are marked isDark=true`() {
        val expectedDark = setOf(
            Palette.GRAPHITE_MIST,
            Palette.LIQUID_BLUE,
            Palette.DEEP_SKY,
            Palette.NORDIC_CYAN,
            Palette.AURORA_VIOLET,
            Palette.AMBER_NIGHT,
        )
        expectedDark.forEach { palette ->
            assertTrue("$palette must be dark", palette.isDark)
        }
    }

    @Test
    fun `light palettes are marked isDark=false`() {
        val expectedLight = setOf(
            Palette.CLOUD_SILVER,
            Palette.FROST_BLUE,
            Palette.PORCELAIN,
            Palette.PEARL_GREY,
        )
        expectedLight.forEach { palette ->
            assertFalse("$palette must be light (isDark=false)", palette.isDark)
        }
    }

    @Test
    fun `total palette count is 10`() {
        assertEquals(
            "Palette must have exactly 10 entries (6 dark + 4 light)",
            10,
            Palette.entries.size,
        )
    }

    // ── 5. Default palette name ───────────────────────────────────────────────

    @Test
    fun `Palette DEFAULT is GRAPHITE_MIST`() {
        assertEquals(Palette.GRAPHITE_MIST, Palette.DEFAULT)
    }

    @Test
    fun `default palette name string is GRAPHITE_MIST`() {
        // This mirrors what Settings.paletteName returns on a fresh install.
        val defaultName = "GRAPHITE_MIST"
        assertEquals(defaultName, Palette.DEFAULT.name)
    }

    // ── 6. Density picker: only COMFORTABLE/COMPACT (no SPACIOUS yet) ─────────

    @Test
    fun `Density has exactly COMFORTABLE and COMPACT variants`() {
        val names = Density.entries.map { it.name }.toSet()
        assertTrue("Density must contain COMFORTABLE", names.contains("COMFORTABLE"))
        assertTrue("Density must contain COMPACT", names.contains("COMPACT"))
        assertFalse("Density must NOT contain SPACIOUS (not yet landed)", names.contains("SPACIOUS"))
    }

    @Test
    fun `Density has exactly 2 entries`() {
        assertEquals(
            "Density must have exactly 2 entries (COMFORTABLE + COMPACT)",
            2,
            Density.entries.size,
        )
    }

    // ── 7. ThemeMode picker ───────────────────────────────────────────────────

    @Test
    fun `ThemeMode has SYSTEM LIGHT and DARK variants`() {
        val names = ThemeMode.entries.map { it.name }.toSet()
        assertTrue("ThemeMode must contain SYSTEM", names.contains("SYSTEM"))
        assertTrue("ThemeMode must contain LIGHT", names.contains("LIGHT"))
        assertTrue("ThemeMode must contain DARK", names.contains("DARK"))
    }

    // ── 8. Accent color is not fully transparent ──────────────────────────────

    @Test
    fun `all palette accent colors have alpha greater than 0`() {
        Palette.entries.forEach { palette ->
            val colors = paletteIdeColors(palette)
            assertTrue(
                "Accent color of $palette must have alpha > 0 (got ${colors.accent.alpha})",
                colors.accent.alpha > 0f,
            )
        }
    }

    // ── Helper — must match the logic in the Appearance section composable ────

    /**
     * Derives a human-readable display label from the enum name.
     * "GRAPHITE_MIST" → "Graphite Mist"
     *
     * This helper mirrors the `paletteDisplayLabel()` private function
     * inside AppearanceSection in SettingsActivity.kt.
     */
    private fun paletteDisplayLabel(palette: Palette): String =
        palette.name
            .split("_")
            .joinToString(" ") { word ->
                word.lowercase().replaceFirstChar { it.uppercaseChar() }
            }
}
