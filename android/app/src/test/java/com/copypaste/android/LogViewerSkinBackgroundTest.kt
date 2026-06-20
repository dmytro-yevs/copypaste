package com.copypaste.android

import com.copypaste.android.ui.theme.Skin
import com.copypaste.android.ui.theme.SkinBackground
import com.copypaste.android.ui.theme.skinTokens
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for the LogViewerActivity skin-background gating logic (A-C6).
 *
 * Mirrors the logic added to LogViewerScreen:
 *   val tok = skinTokens(LocalSkin.current)
 *   val paintAurora = translucent && tok.background == SkinBackground.AURORA
 *   val paintTintBlob = translucent && tok.background == SkinBackground.TINT_BLOB
 *   // Scaffold modifier: aurora → auroraCanvas; tint_blob → auroraCanvas (fallback); flat → none
 *   // containerColor: aurora/tint_blob → Transparent; flat → c.bg
 *
 * No Android SDK or Compose runtime required — only skinTokens() is called.
 */
class LogViewerSkinBackgroundTest {

    /**
     * The aurora-canvas paint gate: translucent AND tok.background == AURORA.
     * CLASSIC is the only skin with AURORA background.
     */
    @Test
    fun `Classic background is AURORA so aurora canvas is painted when translucent`() {
        val tok = skinTokens(Skin.CLASSIC)
        val translucent = true
        val paintAurora = translucent && tok.background == SkinBackground.AURORA
        assertTrue("CLASSIC must paint aurora canvas when translucent", paintAurora)
    }

    @Test
    fun `Classic background is AURORA so aurora canvas is NOT painted when not translucent`() {
        val tok = skinTokens(Skin.CLASSIC)
        val translucent = false
        val paintAurora = translucent && tok.background == SkinBackground.AURORA
        assertFalse("CLASSIC must not paint aurora canvas when translucency is off", paintAurora)
    }

    /**
     * QUIET background is FLAT — no aurora canvas ever, regardless of translucency pref.
     */
    @Test
    fun `Quiet background is FLAT so no aurora canvas is ever painted`() {
        val tok = skinTokens(Skin.QUIET)
        assertEquals(SkinBackground.FLAT, tok.background)
        val translucent = true
        val paintAurora = translucent && tok.background == SkinBackground.AURORA
        assertFalse("QUIET must never paint aurora canvas", paintAurora)
    }

    @Test
    fun `Quiet background is FLAT so no tint-blob canvas is painted either`() {
        val tok = skinTokens(Skin.QUIET)
        val translucent = true
        val paintTintBlob = translucent && tok.background == SkinBackground.TINT_BLOB
        assertFalse("QUIET must not paint a tint-blob canvas", paintTintBlob)
    }

    /**
     * VAPOR background is TINT_BLOB — a static tinted blob canvas (no animated aurora).
     * The implementation falls back to auroraCanvas (TINT_BLOB has no dedicated Modifier yet).
     */
    @Test
    fun `Vapor background is TINT_BLOB so tint-blob path is active when translucent`() {
        val tok = skinTokens(Skin.VAPOR)
        assertEquals(SkinBackground.TINT_BLOB, tok.background)
        val translucent = true
        val paintTintBlob = translucent && tok.background == SkinBackground.TINT_BLOB
        assertTrue("VAPOR must activate tint-blob path when translucent", paintTintBlob)
    }

    @Test
    fun `Vapor background is TINT_BLOB not AURORA`() {
        val tok = skinTokens(Skin.VAPOR)
        assertFalse(
            "VAPOR must not take the aurora path (tok.background != AURORA)",
            tok.background == SkinBackground.AURORA,
        )
    }

    /**
     * containerColor gating:
     *   aurora or tint_blob → Transparent (aurora shows through glass surfaces)
     *   flat                → solid c.bg (no canvas, solid opaque background)
     *
     * Modelled as a pure Boolean: shouldUseTransparentContainer.
     */
    @Test
    fun `Classic container should be transparent when translucent (aurora needs to show)`() {
        val tok = skinTokens(Skin.CLASSIC)
        val translucent = true
        val useTransparent = translucent && tok.background != SkinBackground.FLAT
        assertTrue("CLASSIC with translucency on must use transparent container", useTransparent)
    }

    @Test
    fun `Quiet container must NOT be transparent (flat background requires opaque bg)`() {
        val tok = skinTokens(Skin.QUIET)
        val translucent = true
        // Even when user has translucency on, FLAT background means no canvas → opaque bg.
        val useTransparent = translucent && tok.background != SkinBackground.FLAT
        assertFalse("QUIET must use opaque container (FLAT background)", useTransparent)
    }

    @Test
    fun `Vapor container should be transparent when translucent (tint blob needs to show)`() {
        val tok = skinTokens(Skin.VAPOR)
        val translucent = true
        val useTransparent = translucent && tok.background != SkinBackground.FLAT
        assertTrue("VAPOR with translucency on must use transparent container", useTransparent)
    }

    @Test
    fun `container is always opaque when translucency is off regardless of skin`() {
        val translucent = false
        Skin.entries.forEach { skin ->
            val tok = skinTokens(skin)
            val useTransparent = translucent && tok.background != SkinBackground.FLAT
            assertFalse("$skin must use opaque container when translucency is off", useTransparent)
        }
    }

    /**
     * glow token invariants — LogViewerActivity reads tok.glow for future alpha/effect use.
     */
    @Test
    fun `Classic glow is positive (aurora canvas has non-zero glow)`() {
        assertTrue(skinTokens(Skin.CLASSIC).glow > 0f)
    }

    @Test
    fun `Quiet glow is zero (flat background has no glow)`() {
        assertEquals(0f, skinTokens(Skin.QUIET).glow, 0.001f)
    }

    @Test
    fun `Vapor glow is positive (tint-blob has non-zero glow)`() {
        assertTrue(skinTokens(Skin.VAPOR).glow > 0f)
    }
}
