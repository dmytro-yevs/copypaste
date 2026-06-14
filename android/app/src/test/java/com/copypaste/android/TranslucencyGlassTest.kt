package com.copypaste.android

import com.copypaste.android.ui.theme.GLASS_ALPHA
import com.copypaste.android.ui.theme.GLASS_ALPHA_DARK
import com.copypaste.android.ui.theme.GLASS_ALPHA_LIGHT
import com.copypaste.android.ui.theme.glassAlphaFor
import com.copypaste.android.ui.theme.glassAlphaForTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for the Apple "Liquid Glass" material logic
 * (PARITY-SPEC §2). The glass effect is the alpha applied to surfaces when the
 * translucency pref is ON (default). These are pure functions — no Android
 * SDK, no Compose runtime, no emulator needed.
 */
class TranslucencyGlassTest {

    // ── §2 glass alpha constants (theme-dependent) ──────────────────────────

    @Test
    fun `LIGHT glass alpha is 0_62 per PARITY-SPEC section 2`() {
        assertEquals("light glass alpha must be 0.62f", 0.62f, GLASS_ALPHA_LIGHT, 0.001f)
    }

    @Test
    fun `DARK glass alpha is 0_55 per PARITY-SPEC section 2`() {
        assertEquals("dark glass alpha must be 0.55f", 0.55f, GLASS_ALPHA_DARK, 0.001f)
    }

    @Test
    fun `both glass alphas are strictly between 0 and 1`() {
        assertTrue("light glass alpha must be in (0, 1)", GLASS_ALPHA_LIGHT in 0f..1f && GLASS_ALPHA_LIGHT > 0f && GLASS_ALPHA_LIGHT < 1f)
        assertTrue("dark glass alpha must be in (0, 1)", GLASS_ALPHA_DARK in 0f..1f && GLASS_ALPHA_DARK > 0f && GLASS_ALPHA_DARK < 1f)
    }

    @Test
    fun `default GLASS_ALPHA equals the dark baseline`() {
        assertEquals("GLASS_ALPHA defaults to the dark value", GLASS_ALPHA_DARK, GLASS_ALPHA, 0.001f)
    }

    // ── glassAlphaForTheme(translucent, dark) ───────────────────────────────

    @Test
    fun `glassAlphaForTheme returns light alpha for light translucent`() {
        assertEquals(GLASS_ALPHA_LIGHT, glassAlphaForTheme(translucent = true, dark = false), 0.001f)
    }

    @Test
    fun `glassAlphaForTheme returns dark alpha for dark translucent`() {
        assertEquals(GLASS_ALPHA_DARK, glassAlphaForTheme(translucent = true, dark = true), 0.001f)
    }

    @Test
    fun `glassAlphaForTheme returns 1f (opaque) when translucent is false`() {
        assertEquals(1.0f, glassAlphaForTheme(translucent = false, dark = false), 0.001f)
        assertEquals(1.0f, glassAlphaForTheme(translucent = false, dark = true), 0.001f)
    }

    @Test
    fun `light glass is more opaque than dark glass`() {
        // Warm near-white light glass sits at a higher alpha than the dark fill.
        assertTrue(
            "light glass alpha must exceed dark glass alpha",
            glassAlphaForTheme(translucent = true, dark = false) >
                glassAlphaForTheme(translucent = true, dark = true),
        )
    }

    // ── legacy glassAlphaFor(translucent) shim ──────────────────────────────

    @Test
    fun `glassAlphaFor returns GLASS_ALPHA when translucent is true`() {
        assertEquals(GLASS_ALPHA, glassAlphaFor(translucent = true), 0.001f)
    }

    @Test
    fun `glassAlphaFor returns 1f (fully opaque) when translucent is false`() {
        assertEquals(1.0f, glassAlphaFor(translucent = false), 0.001f)
    }

    @Test
    fun `glassAlphaFor true alpha is less than solid`() {
        assertTrue(
            "translucent alpha must be strictly less than solid (1.0f)",
            glassAlphaFor(translucent = true) < glassAlphaFor(translucent = false),
        )
    }
}
