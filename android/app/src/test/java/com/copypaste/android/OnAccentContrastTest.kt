package com.copypaste.android

import androidx.compose.ui.graphics.Color
import com.copypaste.android.ui.theme.AccentColor
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-eud9 — WCAG AA guard for the on-accent text colour.
 *
 * Every accent×theme cell paints `AccentColor.on(isDark)` text on the filled
 * `AccentColor.base(isDark)` surface (buttons, chips, badges). STYLEGUIDE §3.5
 * claims light "deepens hues to keep AA", but 5/12 cells shipped below 4.5:1
 * (dark blue 3.68, dark rose 3.58, light teal 3.34, light green 3.08,
 * light amber 3.24). This pure-JVM test asserts ALL 12 cells pass AA (>=4.5:1)
 * so the spec defect can never regress.
 */
class OnAccentContrastTest {

    private fun channelToLinear(c: Float): Double {
        val cd = c.toDouble()
        return if (cd <= 0.03928) cd / 12.92 else Math.pow((cd + 0.055) / 1.055, 2.4)
    }

    private fun relativeLuminance(color: Color): Double =
        0.2126 * channelToLinear(color.red) +
            0.7152 * channelToLinear(color.green) +
            0.0722 * channelToLinear(color.blue)

    private fun contrastRatio(a: Color, b: Color): Double {
        val la = relativeLuminance(a)
        val lb = relativeLuminance(b)
        val hi = maxOf(la, lb)
        val lo = minOf(la, lb)
        return (hi + 0.05) / (lo + 0.05)
    }

    @Test
    fun `every accent on-accent text passes WCAG AA on its filled base`() {
        val failures = mutableListOf<String>()
        for (accent in AccentColor.entries) {
            for (isDark in listOf(true, false)) {
                val base = accent.base(isDark)
                val on = accent.on(isDark)
                val ratio = contrastRatio(base, on)
                if (ratio < 4.5) {
                    val theme = if (isDark) "dark" else "light"
                    failures.add("%s %s = %.2f".format(theme, accent.name.lowercase(), ratio))
                }
            }
        }
        assertTrue(
            "on-accent text below WCAG AA (4.5:1) for: $failures",
            failures.isEmpty(),
        )
    }
}
