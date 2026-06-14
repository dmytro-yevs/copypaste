package com.copypaste.android

import com.copypaste.android.ui.theme.GLASS_ALPHA
import com.copypaste.android.ui.theme.glassAlphaFor
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for the translucency/glass logic (CopyPaste-fkj).
 *
 * The glass effect is the alpha applied to panel/elevated surfaces when the
 * translucency pref is ON (default). These are pure functions — no Android
 * SDK, no Compose runtime, no emulator needed.
 */
class TranslucencyGlassTest {

    // ── GLASS_ALPHA constant ────────────────────────────────────────────────

    @Test
    fun `GLASS_ALPHA is approximately 0_72`() {
        // §3 token: container rgba(19,20,26,0.72)
        assertEquals("GLASS_ALPHA must be 0.72f per §3 token", 0.72f, GLASS_ALPHA, 0.001f)
    }

    @Test
    fun `GLASS_ALPHA is strictly between 0 and 1`() {
        assertTrue("GLASS_ALPHA must be in (0, 1)", GLASS_ALPHA > 0f && GLASS_ALPHA < 1f)
    }

    // ── glassAlphaFor(translucent) ──────────────────────────────────────────

    @Test
    fun `glassAlphaFor returns GLASS_ALPHA when translucent is true`() {
        assertEquals(
            "glass alpha when translucent ON must equal GLASS_ALPHA",
            GLASS_ALPHA,
            glassAlphaFor(translucent = true),
            0.001f,
        )
    }

    @Test
    fun `glassAlphaFor returns 1f (fully opaque) when translucent is false`() {
        assertEquals(
            "glass alpha when translucent OFF must be 1.0f (solid)",
            1.0f,
            glassAlphaFor(translucent = false),
            0.001f,
        )
    }

    @Test
    fun `glassAlphaFor true alpha is less than solid`() {
        assertTrue(
            "translucent alpha must be strictly less than solid (1.0f)",
            glassAlphaFor(translucent = true) < glassAlphaFor(translucent = false),
        )
    }

    @Test
    fun `glassAlphaFor false produces fully opaque (1_0f)`() {
        val alpha = glassAlphaFor(translucent = false)
        assertEquals("solid surface must have alpha 1.0f", 1.0f, alpha, 0.0001f)
    }
}
