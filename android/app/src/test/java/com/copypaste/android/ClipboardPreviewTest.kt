package com.copypaste.android

import com.copypaste.android.ClipboardRepository.Companion.PREVIEW_MAX_CHARS
import com.copypaste.android.ClipboardRepository.Companion.previewFromPlaintext
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for [ClipboardRepository.previewFromPlaintext], the helper
 * that turns DECRYPTED clip plaintext into the single-line history-row preview.
 *
 * This is the heart of bug Ac (the history previously showed "(N chars)" for
 * every item because the ciphertext was never decrypted). The decrypt + masking
 * orchestration lives in [ClipboardRepository.parseItem] and needs the Android
 * Base64/UniFFI APIs, but the string-shaping rules are pulled out here as a pure
 * function so they can be verified without an emulator.
 *
 * NOTE: these tests could not be executed in the worktree (no Android SDK /
 * Gradle available here); run them via `./gradlew :app:testDebugUnitTest` on a
 * machine with the SDK.
 */
class ClipboardPreviewTest {

    @Test
    fun normalText_isReturnedVerbatim() {
        assertEquals("Hello world", previewFromPlaintext("Hello world"))
    }

    @Test
    fun whitespace_isCollapsedToSingleLine() {
        assertEquals(
            "line one line two tabbed",
            previewFromPlaintext("line one\n\nline two\t\ttabbed"),
        )
    }

    @Test
    fun leadingAndTrailingWhitespace_isTrimmed() {
        assertEquals("trimmed", previewFromPlaintext("   trimmed   "))
    }

    @Test
    fun longText_isCappedAndEllipsized() {
        val long = "a".repeat(PREVIEW_MAX_CHARS + 50)
        val preview = previewFromPlaintext(long)
        // PREVIEW_MAX_CHARS content chars + the single ellipsis glyph.
        assertEquals(PREVIEW_MAX_CHARS + 1, preview.length)
        assertTrue("should end with ellipsis", preview.endsWith("…"))
    }

    @Test
    fun textExactlyAtCap_isNotEllipsized() {
        val exact = "b".repeat(PREVIEW_MAX_CHARS)
        val preview = previewFromPlaintext(exact)
        assertEquals(exact, preview)
        assertFalse("should not ellipsize at exactly the cap", preview.endsWith("…"))
    }

    @Test
    fun empty_returnsEmpty() {
        assertEquals("", previewFromPlaintext(""))
    }

    @Test
    fun whitespaceOnly_returnsEmpty() {
        assertEquals("", previewFromPlaintext("   \n\t  "))
    }

    @Test
    fun preview_neverContainsNewlines() {
        val preview = previewFromPlaintext("multi\nline\r\nclip\rcontent")
        assertFalse(preview.contains('\n'))
        assertFalse(preview.contains('\r'))
        assertEquals("multi line clip content", preview)
    }
}
