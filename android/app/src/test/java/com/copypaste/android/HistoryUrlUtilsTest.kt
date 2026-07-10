package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * CopyPaste-myh8.5 (S5 5.4, P0-7) — unit tests for [urlPartsForRow], the fix
 * for the partial-span masking leak: the old inline call site fed the RAW
 * (unmasked) display string into the bold-host/dim-path `AnnotatedString`
 * whenever a row's chip label was "URL", bypassing `spanMaskedDisplay`
 * entirely.
 */
class HistoryUrlUtilsTest {

    @Test
    fun `non-URL rows never produce host-path parts`() {
        assertNull(urlPartsForRow(chipLabel = "TEXT", spanMaskedDisplay = null, display = "hello world"))
    }

    @Test
    fun `URL rows with no span masking split the raw display text`() {
        val result = urlPartsForRow(
            chipLabel = "URL",
            spanMaskedDisplay = null,
            display = "https://example.com/path?token=abc",
        )
        assertEquals("example.com" to "/path?token=abc", result)
    }

    @Test
    fun `URL rows with partial sensitive spans split the SPAN-MASKED text, never the raw one`() {
        val secretToken = "SECRET_TOKEN_123"
        val raw = "https://example.com/reset?token=$secretToken"
        val spanMasked = "https://example.com/reset?token=" + "•".repeat(secretToken.length)
        val expectedPath = splitUrl(spanMasked).second

        val result = urlPartsForRow(
            chipLabel = "URL",
            spanMaskedDisplay = spanMasked,
            display = raw,
        )
        requireNotNull(result)
        val (host, path) = result
        assertEquals("example.com", host)
        assertFalse("the sensitive token must never reach the annotated string's path segment", path.contains(secretToken))
        assertEquals(expectedPath, path)
    }
}
