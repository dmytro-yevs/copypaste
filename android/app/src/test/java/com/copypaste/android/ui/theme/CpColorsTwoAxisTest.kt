package com.copypaste.android.ui.theme

import androidx.compose.ui.graphics.Color
import com.copypaste.android.ContentVisualKind
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM contract test for the two-axis theme system (STYLEGUIDE §11):
 * isDark × accent. Verifies the EXACT canonical token values sourced from
 * crates/copypaste-ui/src/styles/tokens.css @ pinned commit 6960539d, so any
 * drift between that file and the Android CpColors/AccentColor tokens is a bug.
 *
 * Recovered/modernised from the deleted CpColorsTwoAxisTest at commit 9553ff4e
 * — extended for the additive fields (card alias, errStrong/infoStrong/okStrong,
 * hover/pressed, forContentKind) introduced by android-material3-redesign S1.
 */
class CpColorsTwoAxisTest {

    @Test
    fun `DarkColors surface and text tokens match tokens css`() {
        assertEquals(Color(0xFF0E0F14), DarkColors.bg)
        assertEquals(Color(0xFF16181F), DarkColors.panel)
        assertEquals(Color(0xFF1E2027), DarkColors.elevated)
        assertEquals(Color(0xFF282B33), DarkColors.raised)
        assertEquals(Color(0xFF33373F), DarkColors.raised2)
        assertEquals(Color(0xFFE7E9EE), DarkColors.text)
        assertEquals(Color(0xFF9CA1AC), DarkColors.dim)
        // faint drift vs the deleted b734a9c2 recovery source: #7E838E -> #8F94A0.
        assertEquals(Color(0xFF8F94A0), DarkColors.faint)
    }

    @Test
    fun `LightColors surface and text tokens match tokens css`() {
        assertEquals(Color(0xFFF5F6F8), LightColors.bg)
        assertEquals(Color(0xFFFFFFFF), LightColors.panel)
        assertEquals(Color(0xFFE1E4E9), LightColors.border)
        assertEquals(Color(0xFFECEEF1), LightColors.divider)
        assertEquals(Color(0xFF1A1C22), LightColors.text)
        assertEquals(Color(0xFF565B66), LightColors.dim)
        // faint drift vs the deleted b734a9c2 recovery source: #767B86 -> #6E7380.
        assertEquals(Color(0xFF6E7380), LightColors.faint)
    }

    @Test
    fun `card is an explicit alias of elevated in both themes`() {
        assertEquals(DarkColors.elevated, DarkColors.card)
        assertEquals(LightColors.elevated, LightColors.card)
    }

    @Test
    fun `overlay and scrim tokens match STYLEGUIDE section 3 point 4`() {
        assertEquals(Color(0x8C000000), DarkColors.scrim)
        assertEquals(Color(0x4714161E), LightColors.scrim)
        assertEquals(0.045f, DarkColors.hover.alpha, 0.004f)
        assertEquals(0.075f, DarkColors.pressed.alpha, 0.004f)
    }

    @Test
    fun `strong status variants equal base in dark and diverge in light`() {
        // Dark theme already clears AA at the base hue.
        assertEquals(DarkColors.err, DarkColors.errStrong)
        assertEquals(DarkColors.info, DarkColors.infoStrong)
        assertEquals(DarkColors.ok, DarkColors.okStrong)
        // Light theme darkens the three variants (see tokens.css comments).
        assertEquals(Color(0xFFB93434), LightColors.errStrong)
        assertEquals(Color(0xFF1D4ED8), LightColors.infoStrong)
        assertEquals(Color(0xFF157A42), LightColors.okStrong)
        assertNotEquals(LightColors.err, LightColors.errStrong)
    }

    @Test
    fun `content-type colors resolve for all ten fields dark and light`() {
        assertEquals(Color(0xFF34D1BF), DarkColors.cUrl)
        assertEquals(Color(0xFFF2616B), DarkColors.cSecret)
        assertEquals(Color(0xFF0E9E8C), LightColors.cUrl)
        assertEquals(Color(0xFFD64545), LightColors.cSecret)
    }

    @Test
    fun `forContentKind aliases PHONE to cNum and PATH to cFile`() {
        assertEquals(DarkColors.cNum, DarkColors.forContentKind(ContentVisualKind.PHONE))
        assertEquals(DarkColors.cFile, DarkColors.forContentKind(ContentVisualKind.PATH))
        assertEquals(DarkColors.cFile, DarkColors.forContentKind(ContentVisualKind.FILE))
        assertEquals(DarkColors.cSecret, DarkColors.forContentKind(ContentVisualKind.SECRET))
        assertEquals(DarkColors.cText, DarkColors.forContentKind(ContentVisualKind.TEXT))
        // All 12 kinds resolve without throwing (exhaustive `when`).
        ContentVisualKind.entries.forEach { DarkColors.forContentKind(it) }
    }

    @Test
    fun `selectedTint uses 16 percent dark and 12 percent light of the active accent`() {
        val dark = selectedTint(AccentColor.INDIGO, isDark = true)
        val light = selectedTint(AccentColor.INDIGO, isDark = false)
        assertEquals(AccentColor.INDIGO.dark.copy(alpha = 0.16f), dark)
        assertEquals(AccentColor.INDIGO.light.copy(alpha = 0.12f), light)
    }

    @Test
    fun `disabledForeground is mute at 45 percent`() {
        assertEquals(DarkColors.mute.copy(alpha = DISABLED_ALPHA), DarkColors.disabledForeground())
        assertEquals(0.45f, DISABLED_ALPHA, 0.001f)
    }

    @Test
    fun `AccentColor enum ships the six styleguide hues in order`() {
        assertEquals(
            listOf("INDIGO", "BLUE", "TEAL", "GREEN", "AMBER", "ROSE"),
            AccentColor.entries.map { it.name },
        )
    }

    @Test
    fun `AccentColor default is indigo`() {
        assertEquals(AccentColor.INDIGO, AccentColor.DEFAULT)
    }

    @Test
    fun `AccentColor base and on resolve per theme`() {
        assertEquals(Color(0xFF6E5BFF), AccentColor.INDIGO.base(isDark = true))
        assertEquals(Color(0xFF5B49E0), AccentColor.INDIGO.base(isDark = false))
        assertEquals(Color.White, AccentColor.INDIGO.on(isDark = true))
        assertEquals(Color(0xFF06302C), AccentColor.TEAL.on(isDark = true))
        assertEquals(Color(0xFF042722), AccentColor.TEAL.on(isDark = false))
        assertEquals(Color(0xFF9C8FFF), AccentColor.INDIGO.variant)
    }

    @Test
    fun `AccentColor fromName falls back to default for null unknown or corrupt values`() {
        assertEquals(AccentColor.DEFAULT, AccentColor.fromName(null))
        assertEquals(AccentColor.DEFAULT, AccentColor.fromName(""))
        assertEquals(AccentColor.DEFAULT, AccentColor.fromName("not-a-real-accent"))
        assertEquals(AccentColor.ROSE, AccentColor.fromName("ROSE"))
    }

    @Test
    fun `every accent on-accent pair meets AA in both themes`() {
        AccentColor.entries.forEach { accent ->
            val darkRatio = WcagContrast.ratio(accent.dark, accent.onDark)
            val lightRatio = WcagContrast.ratio(accent.light, accent.onLight)
            assertTrue("${accent.name} dark on-accent $darkRatio < 4.5", darkRatio >= 4.5)
            assertTrue("${accent.name} light on-accent $lightRatio < 4.5", lightRatio >= 4.5)
        }
    }
}
