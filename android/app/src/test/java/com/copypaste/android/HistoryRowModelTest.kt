package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-vp63.40 — unit tests for the pure derivation logic extracted from
 * HistoryRow.kt into HistoryRowModel.kt.
 *
 * SECURITY (A11Y-1): [resolveRowDisplayText] and [computeMasked] are the single
 * source of truth deciding whether a sensitive clipboard item's plaintext is
 * allowed to reach the UI/a11y tree. The tests below pin that a masked item on
 * a platform that cannot blur (pre-API-31) NEVER returns the real snippet.
 */
class HistoryRowModelTest {

    // ── computeMasked ────────────────────────────────────────────────────────

    @Test
    fun `computeMasked is true when sensitive, mask pref on, and not revealed`() {
        assertTrue(computeMasked(detectedSensitive = true, maskSensitive = true, revealed = false))
    }

    @Test
    fun `computeMasked is false once revealed`() {
        assertFalse(computeMasked(detectedSensitive = true, maskSensitive = true, revealed = true))
    }

    @Test
    fun `computeMasked is false when mask preference is off`() {
        assertFalse(computeMasked(detectedSensitive = true, maskSensitive = false, revealed = false))
    }

    @Test
    fun `computeMasked is false when item is not sensitive`() {
        assertFalse(computeMasked(detectedSensitive = false, maskSensitive = true, revealed = false))
    }

    // ── canBlurSensitiveContent ──────────────────────────────────────────────

    @Test
    fun `canBlurSensitiveContent is true on API 31 and above`() {
        assertTrue(canBlurSensitiveContent(sdkInt = 31))
        assertTrue(canBlurSensitiveContent(sdkInt = 34))
    }

    @Test
    fun `canBlurSensitiveContent is false below API 31`() {
        assertFalse(canBlurSensitiveContent(sdkInt = 30))
        assertFalse(canBlurSensitiveContent(sdkInt = 21))
    }

    // ── resolveRowDisplayText (the core A11Y-1 redaction guarantee) ──────────

    @Test
    fun `masked without blur support never leaks the real snippet`() {
        val secret = "4111 1111 1111 1111"
        val result = resolveRowDisplayText(
            masked = true,
            canBlur = false,
            snippet = secret,
            maskString = "•••• hidden ••••",
            emptyPlaceholder = "(empty)",
        )
        assertEquals("•••• hidden ••••", result)
        assertFalse("masked+no-blur path must never contain the real secret", result.contains(secret))
    }

    @Test
    fun `masked with blur support returns the real snippet for blur to obscure`() {
        val secret = "4111 1111 1111 1111"
        val result = resolveRowDisplayText(
            masked = true,
            canBlur = true,
            snippet = secret,
            maskString = "•••• hidden ••••",
            emptyPlaceholder = "(empty)",
        )
        // The pixel-level blur is applied by the caller (Modifier.blur); the
        // string itself is real here, matching original HistoryRow behavior.
        assertEquals(secret, result)
    }

    @Test
    fun `unmasked blank snippet renders the empty placeholder`() {
        val result = resolveRowDisplayText(
            masked = false,
            canBlur = false,
            snippet = "   ",
            maskString = "•••• hidden ••••",
            emptyPlaceholder = "(empty)",
        )
        assertEquals("(empty)", result)
    }

    @Test
    fun `unmasked non-blank snippet renders as-is`() {
        val result = resolveRowDisplayText(
            masked = false,
            canBlur = false,
            snippet = "hello world",
            maskString = "•••• hidden ••••",
            emptyPlaceholder = "(empty)",
        )
        assertEquals("hello world", result)
    }

    // ── resolveSpanMaskedDisplay ──────────────────────────────────────────────

    @Test
    fun `resolveSpanMaskedDisplay masks only the sensitive sub-range`() {
        val result = resolveSpanMaskedDisplay(
            detectedSensitive = false,
            maskSensitive = true,
            snippet = "call 4111111111111111 now",
            sensitiveSpans = listOf(5..20),
        )
        assertEquals("call •••••••••••••••• now", result)
    }

    @Test
    fun `resolveSpanMaskedDisplay is null for fully-sensitive items (they use full mask instead)`() {
        val result = resolveSpanMaskedDisplay(
            detectedSensitive = true,
            maskSensitive = true,
            snippet = "4111111111111111",
            sensitiveSpans = listOf(0..15),
        )
        assertEquals(null, result)
    }

    @Test
    fun `resolveSpanMaskedDisplay is null when mask preference is off`() {
        val result = resolveSpanMaskedDisplay(
            detectedSensitive = false,
            maskSensitive = false,
            snippet = "call 4111111111111111 now",
            sensitiveSpans = listOf(5..20),
        )
        assertEquals(null, result)
    }

    @Test
    fun `resolveSpanMaskedDisplay is null when there are no spans`() {
        val result = resolveSpanMaskedDisplay(
            detectedSensitive = false,
            maskSensitive = true,
            snippet = "hello world",
            sensitiveSpans = emptyList(),
        )
        assertEquals(null, result)
    }

    // ── shouldSubstitutePlaceholder (shared text/image placeholder gate) ─────

    @Test
    fun `shouldSubstitutePlaceholder is true only when masked and blur is unavailable`() {
        assertTrue(shouldSubstitutePlaceholder(masked = true, canBlur = false))
        assertFalse(shouldSubstitutePlaceholder(masked = true, canBlur = true))
        assertFalse(shouldSubstitutePlaceholder(masked = false, canBlur = false))
        assertFalse(shouldSubstitutePlaceholder(masked = false, canBlur = true))
    }

    // ── shouldHideSemanticsForMasking (A11Y-1 semantics-replacement gate) ────

    @Test
    fun `semantics are hidden only when masked and the platform can blur`() {
        assertTrue(shouldHideSemanticsForMasking(masked = true, canBlur = true))
    }

    @Test
    fun `semantics are not force-hidden when not masked`() {
        assertFalse(shouldHideSemanticsForMasking(masked = false, canBlur = true))
    }

    @Test
    fun `semantics are not force-hidden pre-API-31 (display text already substituted)`() {
        assertFalse(shouldHideSemanticsForMasking(masked = true, canBlur = false))
    }
}
