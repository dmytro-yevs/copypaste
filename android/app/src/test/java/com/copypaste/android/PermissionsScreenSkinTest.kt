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
 * Fix CopyPaste-i1c0: The gating logic is now a 3-way when(tok.background):
 *   AURORA    → aurora canvas (translucent=true gates it); containerColor=Transparent
 *   TINT_BLOB → drawBehind tint-blob with paletteAurora(palette).glowA * tok.glow; containerColor=Transparent
 *   FLAT      → solid c.bg; no canvas modifier
 *
 * CLASSIC must be byte-identical to the pre-skin state (always had aurora canvas).
 * QUIET suppresses the aurora (FLAT background).
 * VAPOR uses TINT_BLOB → inline drawBehind single-blob radial gradient.
 *
 * These are pure-function tests — no Android SDK, no Compose runtime needed.
 */
class PermissionsScreenSkinTest {

    // ── Helper: mirrors the 3-way PermissionsScreen canvas gate ───────────────

    /** Returns which canvas mode would be applied for a given skin + translucency state. */
    private enum class CanvasMode { AURORA, TINT_BLOB, SOLID }

    private fun canvasMode(skin: Skin, translucent: Boolean): CanvasMode {
        val tok = skinTokens(skin)
        return when {
            !translucent -> CanvasMode.SOLID
            tok.background == SkinBackground.AURORA    -> CanvasMode.AURORA
            tok.background == SkinBackground.TINT_BLOB -> CanvasMode.TINT_BLOB
            else                                        -> CanvasMode.SOLID // FLAT
        }
    }

    // ── CLASSIC (A-C8: must be byte-identical to pre-skin state) ──────────────

    @Test
    fun `Classic paints aurora canvas when translucent is ON`() {
        assertEquals(
            "CLASSIC with translucency=ON must paint the aurora canvas (byte-identical to pre-skin state)",
            CanvasMode.AURORA,
            canvasMode(Skin.CLASSIC, translucent = true),
        )
    }

    @Test
    fun `Classic uses solid bg when translucent is OFF`() {
        assertEquals(
            "CLASSIC with translucency=OFF must use solid c.bg (no canvas)",
            CanvasMode.SOLID,
            canvasMode(Skin.CLASSIC, translucent = false),
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
    fun `Quiet always uses solid bg regardless of translucency pref`() {
        assertEquals(
            "QUIET with translucency=ON must use solid bg (FLAT background)",
            CanvasMode.SOLID,
            canvasMode(Skin.QUIET, translucent = true),
        )
        assertEquals(
            "QUIET with translucency=OFF must use solid bg (FLAT background)",
            CanvasMode.SOLID,
            canvasMode(Skin.QUIET, translucent = false),
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

    // ── VAPOR (TINT_BLOB — inline drawBehind radial blob) ─────────────────────

    @Test
    fun `Vapor paints tint-blob canvas when translucent is ON`() {
        assertEquals(
            "VAPOR with translucency=ON must paint the tint-blob canvas (CopyPaste-i1c0)",
            CanvasMode.TINT_BLOB,
            canvasMode(Skin.VAPOR, translucent = true),
        )
    }

    @Test
    fun `Vapor uses solid bg when translucent is OFF`() {
        assertEquals(
            "VAPOR with translucency=OFF must use solid c.bg (no canvas)",
            CanvasMode.SOLID,
            canvasMode(Skin.VAPOR, translucent = false),
        )
    }

    @Test
    fun `Vapor background token is TINT_BLOB`() {
        assertEquals(
            "VAPOR background must be TINT_BLOB so tint-blob gate fires",
            SkinBackground.TINT_BLOB,
            skinTokens(Skin.VAPOR).background,
        )
    }

    @Test
    fun `Vapor never paints aurora canvas`() {
        // TINT_BLOB path is distinct from AURORA — vapor must not use the animated aurora.
        assertFalse(
            "VAPOR must NOT produce AURORA mode (only CLASSIC does that)",
            canvasMode(Skin.VAPOR, translucent = true) == CanvasMode.AURORA,
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
    fun `only VAPOR has TINT_BLOB background`() {
        val tintBlobCount = Skin.entries.count { skin ->
            skinTokens(skin).background == SkinBackground.TINT_BLOB
        }
        assertEquals(
            "Exactly one skin (VAPOR) must have TINT_BLOB background",
            1, tintBlobCount,
        )
        assertTrue(
            "The one TINT_BLOB skin must be VAPOR",
            skinTokens(Skin.VAPOR).background == SkinBackground.TINT_BLOB,
        )
    }

    @Test
    fun `containerColor is transparent only when a canvas is active`() {
        // When canvasMode != SOLID, container should be Transparent.
        // When canvasMode == SOLID, container should be c.bg.
        // Proxy test: verify the gate is consistent for all skins × translucency.
        for (skin in Skin.entries) {
            for (translucent in listOf(true, false)) {
                val mode = canvasMode(skin, translucent)
                val tok = skinTokens(skin)
                when (mode) {
                    CanvasMode.AURORA -> {
                        assertEquals("$skin aurora must have AURORA background",
                            SkinBackground.AURORA, tok.background)
                        assertTrue("aurora must require translucent=true", translucent)
                    }
                    CanvasMode.TINT_BLOB -> {
                        assertEquals("$skin tint-blob must have TINT_BLOB background",
                            SkinBackground.TINT_BLOB, tok.background)
                        assertTrue("tint-blob must require translucent=true", translucent)
                    }
                    CanvasMode.SOLID -> {
                        // Either translucency is off, or background is FLAT.
                        assertTrue(
                            "$skin solid mode must be either !translucent or FLAT background",
                            !translucent || tok.background == SkinBackground.FLAT,
                        )
                    }
                }
            }
        }
    }

    @Test
    fun `aurora gate is disabled when translucent=false even for CLASSIC`() {
        assertEquals(CanvasMode.SOLID, canvasMode(Skin.CLASSIC, translucent = false))
    }

    @Test
    fun `tint-blob gate is disabled when translucent=false even for VAPOR`() {
        assertEquals(CanvasMode.SOLID, canvasMode(Skin.VAPOR, translucent = false))
    }
}
