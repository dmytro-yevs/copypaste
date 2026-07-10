package com.copypaste.android

import androidx.compose.ui.graphics.Color
import com.copypaste.android.ui.theme.AccentColor
import com.copypaste.android.ui.theme.DarkColors
import com.copypaste.android.ui.theme.LightColors
import com.copypaste.android.ui.theme.buildColorScheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-myh8.5 — unit tests for the android-history D2 single content-type
 * color source: [chipLabelFor]'s SECRET override precedence and the new
 * [chipColorFor]\(ContentVisualKind, CpColors\) overload — the function
 * [HistoryRow] now actually renders with, and the one S6 (Preview) should
 * adopt to kill the chip-color divergence the spec calls out.
 */
class HistoryChipsTest {

    // ── chipLabelFor: isSensitive -> SECRET precedence ──────────────────────

    @Test
    fun `sensitive items resolve to the SECRET label regardless of underlying content type`() {
        assertEquals("SECRET", chipLabelFor(contentType = "text/plain", isSensitive = true, snippet = "https://example.com"))
        assertEquals("SECRET", chipLabelFor(contentType = "text/plain", isSensitive = true, snippet = "#ff0000"))
        assertEquals("SECRET", chipLabelFor(contentType = "image/png", isSensitive = true, snippet = ""))
    }

    @Test
    fun `non-sensitive text resolves the same as before (unaffected by the SECRET override)`() {
        assertEquals("IMAGE", chipLabelFor(contentType = "image/png", isSensitive = false, snippet = ""))
        assertEquals("FILE", chipLabelFor(contentType = "application/octet-stream", isSensitive = false, snippet = ""))
        assertEquals("TEXT", chipLabelFor(contentType = "text/plain", isSensitive = false, snippet = ""))
    }

    // ── chipColorFor(ContentVisualKind, CpColors) — the canonical single source ──
    //
    // Fix round (S6 review): a `forContentKind(kind) == chipColorFor(kind, colors)`
    // delegate-equality assertion can never fail — both sides call the exact same
    // one-line function body, so a wrong mapping in [CpColors.forContentKind]
    // itself would sail through undetected. Assert against the literal Color
    // values in ui/theme/Color.kt's DarkColors/LightColors tables instead, so a
    // future edit that breaks the kind->token mapping actually fails this test.

    @Test
    fun `chipColorFor resolves the exact literal content-type tokens in dark theme`() {
        assertEquals(Color(0xFF8B93A5), chipColorFor(ContentVisualKind.TEXT, DarkColors))
        assertEquals(Color(0xFF34D1BF), chipColorFor(ContentVisualKind.URL, DarkColors))
        assertEquals(Color(0xFFE879C6), chipColorFor(ContentVisualKind.IMAGE, DarkColors))
        assertEquals(Color(0xFFF2616B), chipColorFor(ContentVisualKind.SECRET, DarkColors))
    }

    @Test
    fun `chipColorFor resolves the exact literal content-type tokens in light theme`() {
        assertEquals(Color(0xFF6A7282), chipColorFor(ContentVisualKind.TEXT, LightColors))
        assertEquals(Color(0xFF0E9E8C), chipColorFor(ContentVisualKind.URL, LightColors))
        assertEquals(Color(0xFFC44BA0), chipColorFor(ContentVisualKind.IMAGE, LightColors))
        assertEquals(Color(0xFFD64545), chipColorFor(ContentVisualKind.SECRET, LightColors))
    }

    @Test
    fun `SECRET resolves to the dedicated cSecret token, distinct from every other kind`() {
        val secretColor = chipColorFor(ContentVisualKind.SECRET, DarkColors)
        assertEquals(DarkColors.cSecret, secretColor)
        for (kind in ContentVisualKind.entries.filter { it != ContentVisualKind.SECRET }) {
            assertTrue(
                "kind $kind unexpectedly shares SECRET's color",
                chipColorFor(kind, DarkColors) != secretColor,
            )
        }
    }

    // ── chipColorFor(String, ColorScheme) — pre-existing signature stays working ──

    @Test
    fun `legacy String-keyed chipColorFor still resolves every previously-supported label`() {
        val scheme = buildColorScheme(isDark = true, accent = AccentColor.INDIGO)
        for (label in listOf("TEXT", "URL", "EMAIL", "PHONE", "COLOR", "NUMBER", "PATH", "JSON", "CODE", "IMAGE", "FILE")) {
            assertNotNull(chipColorFor(label, scheme))
        }
        // SECRET is additive (defensive parity with the new ContentVisualKind overload).
        assertEquals(scheme.error, chipColorFor("SECRET", scheme))
    }

    // ── parseHexColor — untouched by this slice's changes, sanity-covered ───

    @Test
    fun `parseHexColor extracts the first RGB hex token`() {
        val color = parseHexColor("background: #ff0000;")
        assertNotNull(color)
    }

    @Test
    fun `parseHexColor returns null when no hex token is present`() {
        assertNull(parseHexColor("no color here"))
    }
}
