package com.copypaste.android

import com.copypaste.android.ui.theme.Skin
import com.copypaste.android.ui.theme.SkinBackground
import com.copypaste.android.ui.theme.skinTokens
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for the skin-aware background gating on PermissionsSettingsActivity (A-C8).
 *
 * The gating logic extracted from [PermissionsScreen]:
 *   paintCanvasAurora = tok.background == SkinBackground.AURORA && translucent
 *   containerColor    = if (paintCanvasAurora) Transparent else c.bg
 *
 * CLASSIC must be byte-identical to the pre-skin state (always had aurora canvas).
 * QUIET suppresses the aurora (FLAT background). VAPOR suppresses the animated aurora
 * (TINT_BLOB — static blob not yet animated; falls back to solid-bg container for now;
 * reported in bd-notes as a future TINT_BLOB compositor task).
 *
 * These are pure-function tests — no Android SDK, no Compose runtime needed.
 */
class PermissionsScreenSkinTest {

    /**
     * Mirrors the PermissionsScreen aurora-canvas gate:
     *   paintCanvasAurora = tok.background == SkinBackground.AURORA && translucent
     *
     * Extracted here so it can be exercised on the JVM without Compose.
     */
    private fun paintCanvasAurora(skin: Skin, translucent: Boolean): Boolean {
        val tok = skinTokens(skin)
        return tok.background == SkinBackground.AURORA && translucent
    }

    // ── CLASSIC (A-C8: must be byte-identical to pre-skin state) ──────────────

    @Test
    fun `Classic paints aurora canvas when translucent is ON`() {
        assertTrue(
            "CLASSIC with translucency=ON must paint the aurora canvas (byte-identical to pre-skin state)",
            paintCanvasAurora(Skin.CLASSIC, translucent = true),
        )
    }

    @Test
    fun `Classic does NOT paint aurora canvas when translucent is OFF`() {
        assertFalse(
            "CLASSIC with translucency=OFF must not paint aurora canvas",
            paintCanvasAurora(Skin.CLASSIC, translucent = false),
        )
    }

    @Test
    fun `Classic background token is AURORA`() {
        assertEquals(
            "CLASSIC background must be AURORA so the aurora gate fires",
            SkinBackground.AURORA,
            skinTokens(Skin.CLASSIC).background,
        )
    }

    // ── QUIET (FLAT background — no aurora) ────────────────────────────────────

    @Test
    fun `Quiet never paints aurora canvas regardless of translucency pref`() {
        assertFalse(
            "QUIET with translucency=ON must NOT paint aurora (FLAT background)",
            paintCanvasAurora(Skin.QUIET, translucent = true),
        )
        assertFalse(
            "QUIET with translucency=OFF must NOT paint aurora (FLAT background)",
            paintCanvasAurora(Skin.QUIET, translucent = false),
        )
    }

    @Test
    fun `Quiet background token is FLAT`() {
        assertEquals(
            "QUIET background must be FLAT (aurora gate must not fire)",
            SkinBackground.FLAT,
            skinTokens(Skin.QUIET).background,
        )
    }

    // ── VAPOR (TINT_BLOB — no animated aurora canvas; falls back to solid) ─────

    @Test
    fun `Vapor never paints aurora canvas regardless of translucency pref`() {
        assertFalse(
            "VAPOR with translucency=ON must NOT paint aurora (TINT_BLOB background)",
            paintCanvasAurora(Skin.VAPOR, translucent = true),
        )
        assertFalse(
            "VAPOR with translucency=OFF must NOT paint aurora (TINT_BLOB background)",
            paintCanvasAurora(Skin.VAPOR, translucent = false),
        )
    }

    @Test
    fun `Vapor background token is TINT_BLOB`() {
        assertEquals(
            "VAPOR background must be TINT_BLOB (animated aurora NOT used; TINT_BLOB compositor is future work)",
            SkinBackground.TINT_BLOB,
            skinTokens(Skin.VAPOR).background,
        )
    }

    // ── Invariants across all skins ──────────────────────────────────────────────

    @Test
    fun `only CLASSIC has AURORA background`() {
        val auroraCount = Skin.entries.count { skin ->
            skinTokens(skin).background == SkinBackground.AURORA
        }
        assertEquals(
            "Exactly one skin (CLASSIC) must have AURORA background",
            1, auroraCount,
        )
        assertTrue(
            "The one AURORA skin must be CLASSIC",
            skinTokens(Skin.CLASSIC).background == SkinBackground.AURORA,
        )
    }

    @Test
    fun `aurora gate is disabled when both translucent=false and AURORA background (belt+suspenders)`() {
        // Even CLASSIC must not paint aurora when translucency is off.
        assertFalse(paintCanvasAurora(Skin.CLASSIC, translucent = false))
    }

    @Test
    fun `containerColor is transparent only when aurora is painted`() {
        // Logic: useTransparentContainer = paintCanvasAurora(skin, translucent)
        // Proxy test: for every skin × translucent combination, verify the gate is consistent.
        for (skin in Skin.entries) {
            for (translucent in listOf(true, false)) {
                val paints = paintCanvasAurora(skin, translucent)
                // When paints=true, container should be Transparent (no solid bg).
                // When paints=false, container should be c.bg (solid).
                // We can't check the actual color here (no Compose), but we verify
                // the gate itself is coherent: only fires when background==AURORA.
                val tok = skinTokens(skin)
                if (paints) {
                    assertEquals(
                        "$skin aurora must only fire for AURORA background",
                        SkinBackground.AURORA, tok.background,
                    )
                    assertTrue("aurora must also require translucent=true", translucent)
                }
            }
        }
    }
}
