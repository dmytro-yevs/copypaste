package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-myh8.5 — unit tests for [sanitizedMaskRepresentation], the
 * geometry-preserving, never-plaintext base layer for the pre-API-31 masked
 * row fallback (android-history "List Masking Contract").
 */
class HistoryMaskingTest {

    @Test
    fun `sanitized representation never contains any character of the real snippet`() {
        val secret = "4111 1111 1111 1111"
        val result = sanitizedMaskRepresentation(secret)
        for (ch in secret) {
            if (ch != ' ' && ch != MASK_GLYPH) {
                assertFalse("sanitized output must not contain '$ch' from the real snippet", result.contains(ch))
            }
        }
    }

    @Test
    fun `sanitized representation is built entirely from the mask glyph`() {
        val result = sanitizedMaskRepresentation("hunter2")
        assertTrue(result.all { it == MASK_GLYPH })
    }

    @Test
    fun `sanitized representation tracks the real snippet length`() {
        assertEquals(5, sanitizedMaskRepresentation("hello").length)
        assertEquals(1, sanitizedMaskRepresentation("x").length)
    }

    @Test
    fun `sanitized representation is capped so an enormous secret cannot blow up layout`() {
        val huge = "x".repeat(10_000)
        val result = sanitizedMaskRepresentation(huge)
        assertEquals(MASK_REPRESENTATION_MAX_LEN, result.length)
    }

    @Test
    fun `blank snippet still renders a non-empty sanitized representation`() {
        assertEquals(1, sanitizedMaskRepresentation("").length)
        assertTrue(sanitizedMaskRepresentation("").all { it == MASK_GLYPH })
    }
}
