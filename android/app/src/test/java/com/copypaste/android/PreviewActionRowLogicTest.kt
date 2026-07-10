package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * android-preview S6.2 — unit test for [previewPlaintextExposed], the pure
 * gating predicate behind PreviewActionRow's Reveal/Copy swap and the Open/
 * Save withholding (spec.md "Preview Peeking and Pinned phases — Scenario:
 * Actions toolbar availability").
 */
class PreviewActionRowLogicTest {

    @Test
    fun `non-sensitive items always expose plaintext actions`() {
        assertTrue(previewPlaintextExposed(isSensitive = false, revealed = false))
        assertTrue(previewPlaintextExposed(isSensitive = false, revealed = true))
    }

    @Test
    fun `sensitive and not yet revealed withholds plaintext actions`() {
        assertFalse(previewPlaintextExposed(isSensitive = true, revealed = false))
    }

    @Test
    fun `sensitive and revealed exposes plaintext actions`() {
        assertTrue(previewPlaintextExposed(isSensitive = true, revealed = true))
    }
}
