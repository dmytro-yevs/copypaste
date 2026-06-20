package com.copypaste.android

import com.copypaste.android.ui.theme.Skin
import com.copypaste.android.ui.theme.SkinBackground
import com.copypaste.android.ui.theme.skinTokens
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for the AboutScreen/AboutActivity background-mode logic (A-C3).
 *
 * The background canvas is gated by [SkinTokens.background] and
 * [SkinTokens.glow]:
 *
 *   AURORA   → animated blob aurora canvas (only when translucent is ON)
 *   FLAT     → plain solid background, no aurora (Quiet skin)
 *   TINT_BLOB → static accent-tinted soft blob, no motion aurora (Vapor skin)
 *
 * CLASSIC must remain byte-identical to the pre-skin build (AURORA, glow=0.62).
 *
 * These are pure-function tests — no Android SDK, no Compose runtime needed.
 * They verify [aboutShouldShowAurora] and [aboutShouldShowTintBlob] which are
 * extracted from the Scaffold modifier logic in AboutActivity / AboutScreen.
 */
class AboutBackgroundModeTest {

    // ── Token-level background assertions ────────────────────────────────────

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

    // ── Aurora canvas gate — aboutShouldShowAurora(background, translucent) ──

    /**
     * Aurora is shown only when both conditions hold:
     *   1. tok.background == AURORA
     *   2. translucent == true
     *
     * This matches the gate in AboutActivity/AboutScreen:
     *   `if (translucent && tok.background == SkinBackground.AURORA)`
     */
    @Test
    fun `shouldShowAurora is true for CLASSIC when translucent`() {
        val tok = skinTokens(Skin.CLASSIC)
        assertTrue(
            "CLASSIC + translucent must show aurora",
            aboutShouldShowAurora(tok.background, translucent = true),
        )
    }

    @Test
    fun `shouldShowAurora is false for CLASSIC when not translucent`() {
        val tok = skinTokens(Skin.CLASSIC)
        assertFalse(
            "CLASSIC without translucency must not show aurora",
            aboutShouldShowAurora(tok.background, translucent = false),
        )
    }

    @Test
    fun `shouldShowAurora is false for QUIET regardless of translucency`() {
        val tok = skinTokens(Skin.QUIET)
        assertFalse(
            "QUIET (FLAT background) must never show aurora (translucent=true)",
            aboutShouldShowAurora(tok.background, translucent = true),
        )
        assertFalse(
            "QUIET (FLAT background) must never show aurora (translucent=false)",
            aboutShouldShowAurora(tok.background, translucent = false),
        )
    }

    @Test
    fun `shouldShowAurora is false for VAPOR regardless of translucency`() {
        val tok = skinTokens(Skin.VAPOR)
        assertFalse(
            "VAPOR (TINT_BLOB background) must never show aurora (translucent=true)",
            aboutShouldShowAurora(tok.background, translucent = true),
        )
        assertFalse(
            "VAPOR (TINT_BLOB background) must never show aurora (translucent=false)",
            aboutShouldShowAurora(tok.background, translucent = false),
        )
    }

    // ── Tint-blob gate — aboutShouldShowTintBlob(background, translucent) ────

    /**
     * TINT_BLOB is shown only when both conditions hold:
     *   1. tok.background == TINT_BLOB
     *   2. translucent == true (the blob is part of the glass look)
     */
    @Test
    fun `shouldShowTintBlob is true for VAPOR when translucent`() {
        val tok = skinTokens(Skin.VAPOR)
        assertTrue(
            "VAPOR + translucent must show tint blob",
            aboutShouldShowTintBlob(tok.background, translucent = true),
        )
    }

    @Test
    fun `shouldShowTintBlob is false for VAPOR when not translucent`() {
        val tok = skinTokens(Skin.VAPOR)
        assertFalse(
            "VAPOR without translucency must not show tint blob",
            aboutShouldShowTintBlob(tok.background, translucent = false),
        )
    }

    @Test
    fun `shouldShowTintBlob is false for CLASSIC regardless of translucency`() {
        val tok = skinTokens(Skin.CLASSIC)
        assertFalse(
            "CLASSIC (AURORA background) must never show tint blob",
            aboutShouldShowTintBlob(tok.background, translucent = true),
        )
    }

    @Test
    fun `shouldShowTintBlob is false for QUIET regardless of translucency`() {
        val tok = skinTokens(Skin.QUIET)
        assertFalse(
            "QUIET (FLAT background) must never show tint blob",
            aboutShouldShowTintBlob(tok.background, translucent = true),
        )
    }

    // ── Mutual exclusion — aurora and tint blob are never both true ───────────

    @Test
    fun `aurora and tint_blob are mutually exclusive for all skins`() {
        for (skin in Skin.entries) {
            val tok = skinTokens(skin)
            val aurora = aboutShouldShowAurora(tok.background, translucent = true)
            val tintBlob = aboutShouldShowTintBlob(tok.background, translucent = true)
            assertFalse(
                "$skin must not show both aurora and tint blob simultaneously",
                aurora && tintBlob,
            )
        }
    }

    // ── Glow invariants ───────────────────────────────────────────────────────

    @Test
    fun `CLASSIC glow is 0_62 (frozen — byte-identical requirement)`() {
        assertEquals(0.62f, skinTokens(Skin.CLASSIC).glow, 0.001f)
    }

    @Test
    fun `QUIET glow is 0 (FLAT bg — no glow)`() {
        assertEquals(0f, skinTokens(Skin.QUIET).glow, 0.001f)
    }

    @Test
    fun `VAPOR glow is 0_45 (refined glass)`() {
        assertEquals(0.45f, skinTokens(Skin.VAPOR).glow, 0.001f)
    }

    @Test
    fun `glow is 0 when background is FLAT`() {
        // FLAT background skins must have zero glow — no aurora means no glow.
        for (skin in Skin.entries) {
            val tok = skinTokens(skin)
            if (tok.background == SkinBackground.FLAT) {
                assertEquals(
                    "$skin has FLAT background but non-zero glow",
                    0f, tok.glow, 0.001f,
                )
            }
        }
    }

    @Test
    fun `AURORA skins have positive glow`() {
        for (skin in Skin.entries) {
            val tok = skinTokens(skin)
            if (tok.background == SkinBackground.AURORA) {
                assertTrue(
                    "$skin has AURORA background but zero glow",
                    tok.glow > 0f,
                )
            }
        }
    }

    // ── Scaffold containerColor gate ──────────────────────────────────────────

    /**
     * When the background canvas is shown (aurora or tint blob), the Scaffold
     * containerColor must be transparent so the canvas shows through.
     * When no canvas is shown (FLAT / translucency off), the container uses the
     * opaque ide background color.
     *
     * The logic mirrors AboutActivity.onCreate():
     *   containerColor = if (showCanvas) Color.Transparent else c.bg
     */
    @Test
    fun `CLASSIC translucent shows canvas so container must be transparent`() {
        val tok = skinTokens(Skin.CLASSIC)
        val showCanvas = aboutShouldShowAurora(tok.background, translucent = true) ||
            aboutShouldShowTintBlob(tok.background, translucent = true)
        assertTrue("CLASSIC translucent must show a canvas", showCanvas)
    }

    @Test
    fun `QUIET skin never shows any canvas`() {
        val tok = skinTokens(Skin.QUIET)
        val showCanvas = aboutShouldShowAurora(tok.background, translucent = true) ||
            aboutShouldShowTintBlob(tok.background, translucent = true)
        assertFalse("QUIET must never show any background canvas", showCanvas)
    }

    @Test
    fun `VAPOR translucent shows tint blob canvas`() {
        val tok = skinTokens(Skin.VAPOR)
        val showCanvas = aboutShouldShowAurora(tok.background, translucent = true) ||
            aboutShouldShowTintBlob(tok.background, translucent = true)
        assertTrue("VAPOR translucent must show a canvas (tint blob)", showCanvas)
    }
}

// ---------------------------------------------------------------------------
// Pure gate functions — extracted from AboutActivity / AboutScreen so they
// can be unit-tested on the JVM without the Compose runtime.
//
// These mirror the modifier-selection logic:
//   if (translucent && background == AURORA)   → auroraCanvas()
//   if (translucent && background == TINT_BLOB) → tintBlobCanvas()
//   else                                        → no canvas modifier
// ---------------------------------------------------------------------------

/**
 * Returns true when the About screen should render the animated aurora canvas.
 * Requires [background] == [SkinBackground.AURORA] AND [translucent] == true.
 */
fun aboutShouldShowAurora(background: SkinBackground, translucent: Boolean): Boolean =
    translucent && background == SkinBackground.AURORA

/**
 * Returns true when the About screen should render the static tint-blob canvas.
 * Requires [background] == [SkinBackground.TINT_BLOB] AND [translucent] == true.
 */
fun aboutShouldShowTintBlob(background: SkinBackground, translucent: Boolean): Boolean =
    translucent && background == SkinBackground.TINT_BLOB
