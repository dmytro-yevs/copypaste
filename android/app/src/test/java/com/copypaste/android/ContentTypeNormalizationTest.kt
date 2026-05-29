package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Regression for the Android→peer "ZERO items sent" bug.
 *
 * Stored items use `content_type = "text/plain"` (see [ClipboardRepository]
 * `encodeItem`). The Rust FFI `sync_with_peer` only re-keys and offers items
 * whose content type is the canonical "text" token, so the value handed across
 * the sync boundary (`localItemsForSync`) MUST be normalized — otherwise every
 * Android item is silently filtered out and `items_sent` is 0.
 *
 * This unit test pins the normalization contract that `localItemsForSync`
 * relies on. It is a pure-JVM test (no Android framework deps), runnable via
 * `./gradlew test`.
 */
class ContentTypeNormalizationTest {
    @Test
    fun textPlainNormalizesToText() {
        assertEquals(
            "text",
            ClipboardRepository.normalizeContentTypeForSync("text/plain"),
        )
    }

    @Test
    fun anyTextSubtypeNormalizesToText() {
        assertEquals(
            "text",
            ClipboardRepository.normalizeContentTypeForSync("text/html"),
        )
    }

    @Test
    fun canonicalTextPassesThrough() {
        assertEquals(
            "text",
            ClipboardRepository.normalizeContentTypeForSync("text"),
        )
    }

    @Test
    fun nonTextTypeIsLeftUnchanged() {
        assertEquals(
            "image/png",
            ClipboardRepository.normalizeContentTypeForSync("image/png"),
        )
    }
}
