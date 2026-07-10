package com.copypaste.android

import com.copypaste.android.ui.theme.DarkColors
import com.copypaste.android.ui.theme.LightColors
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * S6 fix round — parity guard the review found missing: asserts the History
 * list row's chip color (`chipColorFor(ContentVisualKind.resolve(...), cp)`,
 * HistoryRow.kt:341-344) and the full-screen Preview chip's color
 * ([previewChipColor], PreviewChrome.kt) resolve to the EXACT SAME [Color]
 * for the same item, in both themes. Before the fix, Preview called the
 * legacy `chipColorFor(String, ColorScheme)` overload against stock M3 hues
 * while the List called `chipColorFor(ContentVisualKind, CpColors)` — a real
 * divergence this test would have caught.
 */
class HistoryRowChipColorParityTest {

    private fun historyRowChipColor(contentType: String, isSensitive: Boolean, snippet: String, colors: com.copypaste.android.ui.theme.CpColors) =
        chipColorFor(ContentVisualKind.resolve(contentType, isSensitive, snippet), colors)

    @Test
    fun `Preview and List resolve the identical color for a plain text item`() {
        for (colors in listOf(DarkColors, LightColors)) {
            val expected = historyRowChipColor("text/plain", isSensitive = false, snippet = "hello world", colors = colors)
            val actual = previewChipColor("text/plain", isSensitive = false, snippet = "hello world", colors = colors)
            assertEquals(expected, actual)
        }
    }

    @Test
    fun `Preview and List resolve the identical color for a URL item`() {
        for (colors in listOf(DarkColors, LightColors)) {
            val expected = historyRowChipColor("text/plain", isSensitive = false, snippet = "https://example.com", colors = colors)
            val actual = previewChipColor("text/plain", isSensitive = false, snippet = "https://example.com", colors = colors)
            assertEquals(expected, actual)
        }
    }

    @Test
    fun `Preview and List resolve the identical color for a sensitive (SECRET) item`() {
        for (colors in listOf(DarkColors, LightColors)) {
            val expected = historyRowChipColor("text/plain", isSensitive = true, snippet = "sk-abc123", colors = colors)
            val actual = previewChipColor("text/plain", isSensitive = true, snippet = "sk-abc123", colors = colors)
            assertEquals(expected, actual)
        }
    }

    @Test
    fun `Preview and List resolve the identical color for an image item`() {
        for (colors in listOf(DarkColors, LightColors)) {
            val expected = historyRowChipColor("image/png", isSensitive = false, snippet = "", colors = colors)
            val actual = previewChipColor("image/png", isSensitive = false, snippet = "", colors = colors)
            assertEquals(expected, actual)
        }
    }
}
