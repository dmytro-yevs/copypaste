package com.copypaste.android

import androidx.compose.ui.graphics.Color
import com.copypaste.android.ui.theme.AccentColor
import com.copypaste.android.ui.theme.DarkColors
import com.copypaste.android.ui.theme.LightColors
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Pure-JVM contract test for the two-axis theme system (STYLEGUIDE §11):
 * isDark × accent. Verifies the EXACT canonical token values so any drift
 * between the styleguide and the Android `CpColors`/`AccentColor` tokens is a bug.
 *
 * This replaces the old palette/skin token tests (Phase 7 deletes those).
 */
class CpColorsTwoAxisTest {

    @Test
    fun `DarkColors surface and text tokens match styleguide section 11`() {
        assertEquals(Color(0xFF0E0F14), DarkColors.bg)
        assertEquals(Color(0xFF16181F), DarkColors.panel)
        assertEquals(Color(0xFF1E2027), DarkColors.elevated)
        assertEquals(Color(0xFFE7E9EE), DarkColors.text)
        assertEquals(Color(0xFF9CA1AC), DarkColors.dim)
    }

    @Test
    fun `LightColors surface and text tokens match styleguide section 11`() {
        assertEquals(Color(0xFFF5F6F8), LightColors.bg)
        assertEquals(Color(0xFFFFFFFF), LightColors.panel)
        assertEquals(Color(0xFF1A1C22), LightColors.text)
        assertEquals(Color(0xFF565B66), LightColors.dim)
    }

    @Test
    fun `AccentColor enum ships the six styleguide hues in order`() {
        assertEquals(
            listOf("INDIGO", "BLUE", "TEAL", "GREEN", "AMBER", "ROSE"),
            AccentColor.entries.map { it.name },
        )
    }

    @Test
    fun `AccentColor base and on resolve per theme`() {
        // indigo (default) — dark base #6E5BFF, light base #5B49E0, white on-accent.
        assertEquals(Color(0xFF6E5BFF), AccentColor.INDIGO.base(isDark = true))
        assertEquals(Color(0xFF5B49E0), AccentColor.INDIGO.base(isDark = false))
        assertEquals(Color.White, AccentColor.INDIGO.on(isDark = true))
        // teal — dark on-accent is the deep #06302C; light on-accent is the deep
        // #052824 (CopyPaste-eud9 WCAG AA fix — white was 3.34:1 on the light base).
        assertEquals(Color(0xFF06302C), AccentColor.TEAL.on(isDark = true))
        assertEquals(Color(0xFF052824), AccentColor.TEAL.on(isDark = false))
        // variant (accent-2) for tinted surfaces.
        assertEquals(Color(0xFF9C8FFF), AccentColor.INDIGO.variant)
    }
}
