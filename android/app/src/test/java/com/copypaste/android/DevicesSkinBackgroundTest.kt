package com.copypaste.android

import com.copypaste.android.ui.theme.Skin
import com.copypaste.android.ui.theme.SkinBackground
import com.copypaste.android.ui.theme.SkinRowTreatment
import com.copypaste.android.ui.theme.skinTokens
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM tests for the A-C2 background / row-treatment gating logic added to
 * [DevicesScreen]:
 *
 *  - [shouldPaintAurora] — true only when the skin has an AURORA background
 *    AND the user has translucency on AND paintCanvasBackdrop is enabled.
 *    Classic is the only skin with AURORA → gating preserves byte-identical output.
 *  - [shouldPaintTintBlob] — true only for TINT_BLOB background (Vapor).
 *  - FLAT background (Quiet) → neither aurora nor blob; plain solid bg.
 *  - Row-gap token: Vapor rowGap = 3dp (applied between device rows), Classic = 0.
 *  - Classic and Quiet rowGap = 0dp (flush / divider).
 *
 * All tests are pure-function / pure-Kotlin — no Android SDK required.
 */
class DevicesSkinBackgroundTest {

    // ── shouldPaintAurora ─────────────────────────────────────────────────────

    @Test
    fun `shouldPaintAurora is true for Classic when translucent and paintCanvasBackdrop enabled`() {
        assertTrue(
            "Classic + translucent + paintCanvas → aurora ON",
            shouldPaintAurora(
                background = skinTokens(Skin.CLASSIC).background,
                translucent = true,
                paintCanvasBackdrop = true,
            ),
        )
    }

    @Test
    fun `shouldPaintAurora is false for Classic when paintCanvasBackdrop disabled`() {
        assertFalse(
            "paintCanvasBackdrop=false must suppress aurora even for Classic",
            shouldPaintAurora(
                background = skinTokens(Skin.CLASSIC).background,
                translucent = true,
                paintCanvasBackdrop = false,
            ),
        )
    }

    @Test
    fun `shouldPaintAurora is false for Classic when translucency off`() {
        assertFalse(
            "translucent=false must suppress aurora for Classic",
            shouldPaintAurora(
                background = skinTokens(Skin.CLASSIC).background,
                translucent = false,
                paintCanvasBackdrop = true,
            ),
        )
    }

    @Test
    fun `shouldPaintAurora is false for Quiet (FLAT background)`() {
        val tok = skinTokens(Skin.QUIET)
        assertEquals("Quiet background must be FLAT", SkinBackground.FLAT, tok.background)
        assertFalse(
            "Quiet / FLAT background → no aurora canvas",
            shouldPaintAurora(
                background = tok.background,
                translucent = true,
                paintCanvasBackdrop = true,
            ),
        )
    }

    @Test
    fun `shouldPaintAurora is false for Vapor (TINT_BLOB background)`() {
        val tok = skinTokens(Skin.VAPOR)
        assertEquals("Vapor background must be TINT_BLOB", SkinBackground.TINT_BLOB, tok.background)
        assertFalse(
            "Vapor / TINT_BLOB background → no animated aurora canvas",
            shouldPaintAurora(
                background = tok.background,
                translucent = true,
                paintCanvasBackdrop = true,
            ),
        )
    }

    // ── shouldPaintTintBlob ───────────────────────────────────────────────────

    @Test
    fun `shouldPaintTintBlob is true for Vapor when translucent and paintCanvasBackdrop enabled`() {
        assertTrue(
            "Vapor + translucent + paintCanvas → tint-blob ON",
            shouldPaintTintBlob(
                background = skinTokens(Skin.VAPOR).background,
                translucent = true,
                paintCanvasBackdrop = true,
            ),
        )
    }

    @Test
    fun `shouldPaintTintBlob is false for Vapor when translucency off`() {
        assertFalse(
            "translucent=false → no tint blob even for Vapor",
            shouldPaintTintBlob(
                background = skinTokens(Skin.VAPOR).background,
                translucent = false,
                paintCanvasBackdrop = true,
            ),
        )
    }

    @Test
    fun `shouldPaintTintBlob is false for Classic (AURORA background)`() {
        assertFalse(
            "Classic / AURORA background → no tint blob",
            shouldPaintTintBlob(
                background = skinTokens(Skin.CLASSIC).background,
                translucent = true,
                paintCanvasBackdrop = true,
            ),
        )
    }

    @Test
    fun `shouldPaintTintBlob is false for Quiet (FLAT background)`() {
        assertFalse(
            "Quiet / FLAT background → no tint blob",
            shouldPaintTintBlob(
                background = skinTokens(Skin.QUIET).background,
                translucent = true,
                paintCanvasBackdrop = true,
            ),
        )
    }

    // ── Background enum alignment ─────────────────────────────────────────────

    @Test
    fun `Classic background token is AURORA`() {
        assertEquals(SkinBackground.AURORA, skinTokens(Skin.CLASSIC).background)
    }

    @Test
    fun `Quiet background token is FLAT`() {
        assertEquals(SkinBackground.FLAT, skinTokens(Skin.QUIET).background)
    }

    @Test
    fun `Vapor background token is TINT_BLOB`() {
        assertEquals(SkinBackground.TINT_BLOB, skinTokens(Skin.VAPOR).background)
    }

    // ── Row gap token ─────────────────────────────────────────────────────────

    @Test
    fun `Classic rowGap is 0dp (flush rows - byte-identical)`() {
        assertEquals(0f, skinTokens(Skin.CLASSIC).rowGap.value, 0.01f)
    }

    @Test
    fun `Quiet rowGap is 0dp (LINE rows use dividers not gaps)`() {
        assertEquals(0f, skinTokens(Skin.QUIET).rowGap.value, 0.01f)
    }

    @Test
    fun `Vapor rowGap is 3dp (INSET rows are separated by a gap)`() {
        assertEquals(3f, skinTokens(Skin.VAPOR).rowGap.value, 0.01f)
    }

    // ── Row treatment token alignment ─────────────────────────────────────────

    @Test
    fun `Classic rowTreatment is CARD`() {
        assertEquals(SkinRowTreatment.CARD, skinTokens(Skin.CLASSIC).rowTreatment)
    }

    @Test
    fun `Quiet rowTreatment is LINE`() {
        assertEquals(SkinRowTreatment.LINE, skinTokens(Skin.QUIET).rowTreatment)
    }

    @Test
    fun `Vapor rowTreatment is INSET`() {
        assertEquals(SkinRowTreatment.INSET, skinTokens(Skin.VAPOR).rowTreatment)
    }

    // ── Glow invariants ───────────────────────────────────────────────────────

    @Test
    fun `Classic glow is non-zero (aurora active)`() {
        assertTrue(
            "Classic glow must be > 0 (aurora canvas is live)",
            skinTokens(Skin.CLASSIC).glow > 0f,
        )
    }

    @Test
    fun `Quiet glow is zero (FLAT background — no aurora)`() {
        assertEquals(
            "Quiet glow must be 0 (FLAT background, no aurora)",
            0f,
            skinTokens(Skin.QUIET).glow,
            0.001f,
        )
    }

    @Test
    fun `Vapor glow is non-zero (tint blob is visible)`() {
        assertTrue(
            "Vapor glow must be > 0 (tint blob is shown)",
            skinTokens(Skin.VAPOR).glow > 0f,
        )
    }
}
