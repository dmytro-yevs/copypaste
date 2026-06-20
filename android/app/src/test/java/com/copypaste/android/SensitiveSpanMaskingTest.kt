package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for CopyPaste-ojsh — sensitive-span masking on Android.
 *
 * Covers:
 *  1. [detectSensitiveSpans] wrapper: stub-mode returns empty list (no native .so).
 *  2. [ClipboardItem.sensitiveSpans] field exists and defaults to empty.
 *  3. [applySpanMasking] pure function — mirrors macOS masking.ts semantics.
 *
 * All tests run on the JVM (no Android runtime, no NDK).
 * The native library is NOT loaded ([isNativeLibraryLoaded] == false), so stub paths execute.
 *
 * Run via: ./gradlew :app:testDebugUnitTest --tests "*.SensitiveSpanMaskingTest"
 */
class SensitiveSpanMaskingTest {

    // ── 1. detectSensitiveSpans wrapper — stub mode ───────────────────────────

    @Test
    fun detectSensitiveSpans_stubMode_returnsEmptyList() {
        // isNativeLibraryLoaded is false in JVM unit tests (no .so on classpath).
        // The wrapper must return empty list — never throw and never return plaintext spans.
        val result = detectSensitiveSpans("4111 1111 1111 1111")
        assertNotNull("Should return a non-null list in stub mode", result)
        assertTrue("Stub mode must return empty list (no .so available)", result.isEmpty())
    }

    @Test
    fun detectSensitiveSpans_stubMode_emptyText_returnsEmptyList() {
        val result = detectSensitiveSpans("")
        assertTrue("Empty text stub returns empty list", result.isEmpty())
    }

    // ── 2. ClipboardItem.sensitiveSpans field ─────────────────────────────────

    @Test
    fun clipboardItem_sensitiveSpans_defaultsToEmpty() {
        val item = ClipboardItem(
            id = "test-id",
            contentType = "text",
            isSensitive = false,
            wallTimeMs = 1000L,
            snippet = "Hello 4111 1111 1111 1111 world",
        )
        assertNotNull("sensitiveSpans must not be null", item.sensitiveSpans)
        assertTrue("sensitiveSpans defaults to empty list", item.sensitiveSpans.isEmpty())
    }

    @Test
    fun clipboardItem_sensitiveSpans_canBePopulated() {
        val spans = listOf(6..25)
        val item = ClipboardItem(
            id = "test-id",
            contentType = "text",
            isSensitive = false,
            wallTimeMs = 1000L,
            snippet = "Hello 4111 1111 1111 1111 world",
            sensitiveSpans = spans,
        )
        assertEquals(1, item.sensitiveSpans.size)
        assertEquals(6, item.sensitiveSpans[0].first)
        assertEquals(25, item.sensitiveSpans[0].last)
    }

    @Test
    fun clipboardItem_fullySensitive_sensitiveSpansEmpty() {
        // Fully sensitive items use full-blur masking, not span masking.
        // sensitiveSpans should default to empty for them.
        val item = ClipboardItem(
            id = "sensitive-id",
            contentType = "text",
            isSensitive = true,
            wallTimeMs = 1000L,
            snippet = "4111 1111 1111 1111",
        )
        assertTrue(
            "Fully-sensitive items should have empty sensitiveSpans (full blur applies)",
            item.sensitiveSpans.isEmpty(),
        )
    }

    // ── 3. applySpanMasking pure function ─────────────────────────────────────

    @Test
    fun applySpanMasking_noSpans_returnsOriginalText() {
        val text = "Hello world"
        val result = applySpanMasking(text, emptyList())
        assertEquals("Empty spans should leave text unchanged", text, result)
    }

    @Test
    fun applySpanMasking_singleSpan_masksCorrectRange() {
        // "Hello 4111 1111 1111 1111 world"
        //        6                    25
        val text = "Hello 4111 1111 1111 1111 world"
        // mask from index 6 to 25 (exclusive) — covers the card number
        val result = applySpanMasking(text, listOf(6..24))
        assertTrue("Prefix 'Hello ' should be preserved", result.startsWith("Hello "))
        assertTrue("Suffix ' world' should be preserved", result.endsWith(" world"))
        // The masked region should be all bullets
        val masked = result.substring(6, result.length - " world".length)
        assertTrue("Masked region must contain only bullets", masked.all { it == '•' })
        assertEquals("Bullet count must equal span character count", 19, masked.length)
    }

    @Test
    fun applySpanMasking_multipleSpans_masksAll() {
        // "card: 4111 iban: DE89370400440532013000 end"
        //  0123456789012345678901234567890123456789012
        //  0         1         2         3         4
        // "4111" = positions 6..9 (inclusive)
        // "DE89370400440532013000" = positions 17..38 (inclusive)
        val text = "card: 4111 iban: DE89370400440532013000 end"
        val spans = listOf(6..9, 17..38)
        val result = applySpanMasking(text, spans)
        assertTrue("Prefix 'card: ' must be preserved", result.startsWith("card: "))
        // All bullet characters should appear where spans were
        assertTrue("Result must contain bullets", result.contains('•'))
        // The unmasked parts must still be there
        // " iban: " is at positions 10-16 — between the two masked spans
        assertTrue("' iban: ' between spans must be preserved", result.contains(" iban: "))
        assertTrue("' end' suffix must be preserved", result.endsWith(" end"))
    }

    @Test
    fun applySpanMasking_fullText_masksEntireText() {
        val text = "4111111111111111"
        val spans = listOf(0..16)
        val result = applySpanMasking(text, spans)
        assertTrue("All characters should be masked", result.all { it == '•' })
        assertEquals("Masked length must match original length", text.length, result.length)
    }

    @Test
    fun applySpanMasking_spanBeyondTextLength_clamped() {
        val text = "hello"
        // Span extends well beyond the text
        val spans = listOf(3..100)
        val result = applySpanMasking(text, spans)
        assertEquals("Prefix before span must be preserved", "hel", result.substring(0, 3))
        // Remaining characters (lo) should be masked
        assertEquals("Masked tail must be all bullets", "••", result.substring(3))
    }

    @Test
    fun applySpanMasking_emptyText_returnsEmpty() {
        val result = applySpanMasking("", listOf(0..5))
        assertEquals("Empty text stays empty", "", result)
    }

    @Test
    fun applySpanMasking_unsortedSpans_processedLeftToRight() {
        // Spans provided in reverse order — must still process correctly.
        // "AAA BBB CCC"
        //  0123456789A   (A = 10)
        // "BBB" is at positions 4..6 (inclusive, 3 chars)
        // "CCC" is at positions 8..10 (inclusive, 3 chars)
        val text = "AAA BBB CCC"
        val result = applySpanMasking(text, listOf(8..10, 4..6))
        // Sorted: [4..6, 8..10]. Space at 7 is preserved between masked regions.
        assertEquals("AAA ••• •••", result)
    }

    @Test
    fun applySpanMasking_adjacentSpans_mergedCorrectly() {
        val text = "AABBCC"
        // Two adjacent spans that together cover all of "BBCC"
        val spans = listOf(2..4, 4..6)
        val result = applySpanMasking(text, spans)
        assertEquals("AA", result.substring(0, 2))
        assertEquals("••••", result.substring(2))
    }
}
