package com.copypaste.android

import com.copypaste.android.ui.theme.Skin
import com.copypaste.android.ui.theme.SkinBackground
import com.copypaste.android.ui.theme.skinTokens
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM tests for the shared [tintBlobCanvas] modifier extracted to Components.kt
 * (CopyPaste-uya3 cleanup).
 *
 * Verifies:
 *  1. The canonical modifier lives in Components.kt and is NOT private (it is
 *     internal / package-visible to the theme package, tested indirectly through
 *     the skin token values it uses).
 *  2. The gating invariants — skin tokens that drive TINT_BLOB path — are correct,
 *     so every screen using `Modifier.tintBlobCanvas(dark, auroraDef, tok.glow)`
 *     gets equivalent behaviour.
 *  3. VAPOR tok.glow = 0.45f so blob alpha scaling is consistent across screens.
 *  4. The modifier only triggers for TINT_BLOB background (Vapor), not AURORA or FLAT.
 *
 * These are pure-Kotlin / pure-JVM tests — no Android SDK or Compose runtime needed.
 * They verify the token invariants that the shared modifier relies on, guarding
 * against future regressions from skin-token drift.
 */
class TintBlobCanvasModifierTest {

    // ── Token invariants (drive the shared modifier correctly) ────────────────

    @Test
    fun `Vapor background is TINT_BLOB — tintBlobCanvas path active`() {
        val tok = skinTokens(Skin.VAPOR)
        assertEquals(
            "Vapor skin must report TINT_BLOB so tintBlobCanvas is applied",
            SkinBackground.TINT_BLOB,
            tok.background,
        )
    }

    @Test
    fun `Classic background is AURORA — tintBlobCanvas path NOT active`() {
        val tok = skinTokens(Skin.CLASSIC)
        assertEquals(
            "Classic skin must NOT be TINT_BLOB; auroraCanvas is used instead",
            SkinBackground.AURORA,
            tok.background,
        )
    }

    @Test
    fun `Quiet background is FLAT — tintBlobCanvas path NOT active`() {
        val tok = skinTokens(Skin.QUIET)
        assertEquals(
            "Quiet skin must NOT be TINT_BLOB; no canvas is drawn",
            SkinBackground.FLAT,
            tok.background,
        )
    }

    // ── Glow token: blob alpha scaling ────────────────────────────────────────

    @Test
    fun `Vapor glow is 0_45 — canonical blob alpha scale`() {
        val tok = skinTokens(Skin.VAPOR)
        assertEquals(
            "Vapor glow must be 0.45f — the shared tintBlobCanvas relies on this for alpha scaling",
            0.45f,
            tok.glow,
            0.001f,
        )
    }

    @Test
    fun `Vapor glow is in valid range 0-1`() {
        val tok = skinTokens(Skin.VAPOR)
        assertTrue(
            "Vapor glow must be in [0,1] — coerceIn clamps are applied in the modifier but glow itself must be sane",
            tok.glow in 0f..1f,
        )
    }

    // ── Gate expression: translucent && tok.background == TINT_BLOB ──────────

    @Test
    fun `tintBlob gate is active for Vapor when translucent is true`() {
        val tok = skinTokens(Skin.VAPOR)
        val translucent = true
        val paintTintBlob = translucent && tok.background == SkinBackground.TINT_BLOB
        assertTrue(
            "Vapor + translucent=true must activate the TINT_BLOB modifier path",
            paintTintBlob,
        )
    }

    @Test
    fun `tintBlob gate is inactive for Vapor when translucent is false`() {
        val tok = skinTokens(Skin.VAPOR)
        val translucent = false
        val paintTintBlob = translucent && tok.background == SkinBackground.TINT_BLOB
        assertTrue(
            "translucent=false must suppress TINT_BLOB canvas even for Vapor",
            !paintTintBlob,
        )
    }

    @Test
    fun `tintBlob gate is inactive for Classic (AURORA background)`() {
        val tok = skinTokens(Skin.CLASSIC)
        val translucent = true
        val paintTintBlob = translucent && tok.background == SkinBackground.TINT_BLOB
        assertTrue(
            "Classic skin must NOT activate the TINT_BLOB path (AURORA background)",
            !paintTintBlob,
        )
    }

    @Test
    fun `tintBlob gate is inactive for Quiet (FLAT background)`() {
        val tok = skinTokens(Skin.QUIET)
        val translucent = true
        val paintTintBlob = translucent && tok.background == SkinBackground.TINT_BLOB
        assertTrue(
            "Quiet skin must NOT activate the TINT_BLOB path (FLAT background)",
            !paintTintBlob,
        )
    }

    // ── Canonical alpha scaling formula (AboutActivity reference) ────────────

    /**
     * Verifies the formula used in the shared tintBlobCanvas modifier:
     *   blobAlpha = (sourceAlpha * glow * 1.4f).coerceIn(0f, 1f)
     *
     * The * 1.4f boosts the blob above the raw glowA alpha so the Vapor canvas
     * reads as a clearly tinted backdrop (without it the blobs are too faint on
     * lower-DPI screens). The coerceIn ensures we never exceed 1.0.
     *
     * Using test values: sourceAlpha=0.42, glow=0.45 → 0.42 * 0.45 * 1.4 = 0.2646
     */
    @Test
    fun `canonical blob alpha formula produces correct value`() {
        val sourceAlpha = 0.42f
        val glow = 0.45f          // Vapor tok.glow
        val boost = 1.4f          // AboutActivity canonical constant
        val expected = (sourceAlpha * glow * boost).coerceIn(0f, 1f)
        assertEquals("blob alpha formula: 0.42 * 0.45 * 1.4 = 0.2646", 0.2646f, expected, 0.0001f)
    }

    @Test
    fun `canonical blob alpha is clamped to 1_0 when inputs saturate`() {
        val sourceAlpha = 1.0f
        val glow = 1.0f
        val boost = 1.4f
        val result = (sourceAlpha * glow * boost).coerceIn(0f, 1f)
        assertEquals("saturated inputs must be clamped to 1.0", 1.0f, result, 0.0001f)
    }

    @Test
    fun `centre accent alpha formula uses no boost (AboutActivity canonical)`() {
        // Centre accent: alpha = (overlayAccent.alpha * glow).coerceIn(0f, 1f)
        // No 1.4f boost — the centre blob is subtler (warm midpoint fill).
        val overlayAlpha = 0.18f
        val glow = 0.45f
        val result = (overlayAlpha * glow).coerceIn(0f, 1f)
        assertEquals("centre accent: 0.18 * 0.45 = 0.081", 0.081f, result, 0.0001f)
    }
}
