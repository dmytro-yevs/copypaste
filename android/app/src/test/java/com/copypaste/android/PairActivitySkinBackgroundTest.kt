package com.copypaste.android

import com.copypaste.android.ui.theme.Skin
import com.copypaste.android.ui.theme.SkinBackground
import com.copypaste.android.ui.theme.skinTokens
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM tests for the skin-based aurora canvas gating added in A-C5.
 *
 * Verifies the tok.background == SkinBackground.AURORA gate that controls whether
 * PairScreen renders the aurora canvas backdrop. CLASSIC must produce AURORA
 * (byte-identical to pre-skin behaviour); QUIET must produce FLAT (no aurora);
 * VAPOR must produce TINT_BLOB (no aurora — static blob, not the animated canvas).
 *
 * The gating expression in PairScreen is:
 *   translucent && tok.background == SkinBackground.AURORA
 *
 * No Android Context or Compose runtime is required — skinTokens() is a pure
 * registry lookup.
 */
class PairActivitySkinBackgroundTest {

    // ── SkinBackground values per skin ────────────────────────────────────────

    @Test
    fun `CLASSIC skin has AURORA background — aurora canvas is shown`() {
        val tok = skinTokens(Skin.CLASSIC)
        assertEquals(
            "CLASSIC skin must use SkinBackground.AURORA so aurora canvas appears (byte-identical to pre-skin build)",
            SkinBackground.AURORA,
            tok.background,
        )
    }

    @Test
    fun `QUIET skin has FLAT background — aurora canvas is suppressed`() {
        val tok = skinTokens(Skin.QUIET)
        assertEquals(
            "QUIET skin must use SkinBackground.FLAT so no aurora canvas is drawn",
            SkinBackground.FLAT,
            tok.background,
        )
    }

    @Test
    fun `VAPOR skin has TINT_BLOB background — aurora canvas is suppressed`() {
        val tok = skinTokens(Skin.VAPOR)
        assertEquals(
            "VAPOR skin must use SkinBackground.TINT_BLOB so no aurora canvas is drawn",
            SkinBackground.TINT_BLOB,
            tok.background,
        )
    }

    // ── Aurora gate expression: translucent && tok.background == AURORA ───────

    @Test
    fun `CLASSIC with translucency on — gate is true (aurora shows)`() {
        val tok = skinTokens(Skin.CLASSIC)
        val translucent = true
        val paintAurora = translucent && tok.background == SkinBackground.AURORA
        assertTrue(
            "CLASSIC + translucent=true must enable aurora canvas",
            paintAurora,
        )
    }

    @Test
    fun `CLASSIC with translucency off — gate is false (no aurora)`() {
        val tok = skinTokens(Skin.CLASSIC)
        val translucent = false
        val paintAurora = translucent && tok.background == SkinBackground.AURORA
        assertFalse(
            "CLASSIC + translucent=false must suppress aurora canvas (translucency pref overrides skin)",
            paintAurora,
        )
    }

    @Test
    fun `QUIET with translucency on — gate is false (no aurora, FLAT background)`() {
        val tok = skinTokens(Skin.QUIET)
        val translucent = true
        val paintAurora = translucent && tok.background == SkinBackground.AURORA
        assertFalse(
            "QUIET + translucent=true must suppress aurora canvas (FLAT background, not AURORA)",
            paintAurora,
        )
    }

    @Test
    fun `VAPOR with translucency on — gate is false (no aurora, TINT_BLOB background)`() {
        val tok = skinTokens(Skin.VAPOR)
        val translucent = true
        val paintAurora = translucent && tok.background == SkinBackground.AURORA
        assertFalse(
            "VAPOR + translucent=true must suppress aurora canvas (TINT_BLOB, not AURORA)",
            paintAurora,
        )
    }

    // ── Glow token is available per skin ─────────────────────────────────────

    @Test
    fun `CLASSIC glow is positive (0_62)`() {
        val tok = skinTokens(Skin.CLASSIC)
        assertTrue(
            "CLASSIC glow must be positive — aurora blobs contribute chromatic glow",
            tok.glow > 0f,
        )
        assertEquals("CLASSIC glow is 0.62f", 0.62f, tok.glow, 0.001f)
    }

    @Test
    fun `QUIET glow is zero — no glow effects`() {
        val tok = skinTokens(Skin.QUIET)
        assertEquals(
            "QUIET glow must be 0f — flat skin has no glow effects",
            0f,
            tok.glow,
            0.001f,
        )
    }

    @Test
    fun `VAPOR glow is 0_45`() {
        val tok = skinTokens(Skin.VAPOR)
        assertEquals("VAPOR glow is 0.45f", 0.45f, tok.glow, 0.001f)
    }
}
