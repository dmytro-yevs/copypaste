package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for [SyncFileHelper] and file-related helpers.
 *
 * Tests verify:
 *   1. [SyncFileHelper.buildFileLabel] constructs a "[file: <name>]" label from fileName.
 *   2. Falls back to "[file]" when fileName is null/blank.
 *   3. [ClipboardItem.isFile] returns true only for content_type=="file".
 *   4. [SyncFileHelper.storeAndLabel] callback contract:
 *      - invoked with bytes when bytes are non-empty
 *      - returns the label string
 *      - NOT invoked (returns null) when bytes are empty
 */
class SyncFileHelperTest {

    // ── buildFileLabel ────────────────────────────────────────────────────────

    @Test
    fun buildFileLabel_withFileName_returnsLabelWithName() {
        val label = SyncFileHelper.buildFileLabel("report.pdf")
        assertEquals("[file: report.pdf]", label)
    }

    @Test
    fun buildFileLabel_withBlankName_returnsPlaceholder() {
        val label = SyncFileHelper.buildFileLabel("   ")
        assertEquals("[file]", label)
    }

    @Test
    fun buildFileLabel_withNull_returnsPlaceholder() {
        val label = SyncFileHelper.buildFileLabel(null)
        assertEquals("[file]", label)
    }

    // ── ClipboardItem.isFile ──────────────────────────────────────────────────

    @Test
    fun clipboardItem_isFile_trueForFileContentType() {
        val item = ClipboardItem(
            id = "abc",
            contentType = "file",
            isSensitive = false,
            wallTimeMs = 0L,
        )
        assertTrue("isFile must be true for contentType=file", item.isFile)
    }

    @Test
    fun clipboardItem_isFile_falseForText() {
        val item = ClipboardItem(
            id = "abc",
            contentType = "text/plain",
            isSensitive = false,
            wallTimeMs = 0L,
        )
        assertFalse("isFile must be false for text/plain", item.isFile)
    }

    @Test
    fun clipboardItem_isFile_falseForImage() {
        val item = ClipboardItem(
            id = "abc",
            contentType = "image/png",
            isSensitive = false,
            wallTimeMs = 0L,
        )
        assertFalse("isFile must be false for image/png", item.isFile)
    }

    // ── SyncFileHelper.storeAndLabel ──────────────────────────────────────────

    @Test
    fun storeAndLabel_withBytes_callsStoreAndReturnsLabel() {
        val bytes = ByteArray(16) { it.toByte() }
        var storedBytes: ByteArray? = null
        var storedMeta: Pair<String?, String?>? = null

        val label = SyncFileHelper.storeAndLabel(
            fileBytes = bytes,
            fileName = "data.bin",
            mime = "application/octet-stream",
            storeBytes = { b -> storedBytes = b },
            storeMeta = { fn, m -> storedMeta = fn to m },
        )

        assertEquals("[file: data.bin]", label)
        assertTrue("storeBytes must be called with original bytes", nullSafeContentEquals(storedBytes, bytes))
        assertEquals("data.bin" to "application/octet-stream", storedMeta)
    }

    @Test
    fun storeAndLabel_withEmptyBytes_returnsNullAndDoesNotCallStore() {
        var storeCalled = false

        val label = SyncFileHelper.storeAndLabel(
            fileBytes = ByteArray(0),
            fileName = "empty.bin",
            mime = null,
            storeBytes = { _ -> storeCalled = true },
            storeMeta = { _, _ -> storeCalled = true },
        )

        assertNull("storeAndLabel must return null when bytes are empty", label)
        assertFalse("store callbacks must NOT be invoked for empty bytes", storeCalled)
    }

    @Test
    fun storeAndLabel_withNullFileName_usesPlaceholderLabel() {
        val bytes = ByteArray(4) { 0 }
        var storedMeta: Pair<String?, String?>? = null

        val label = SyncFileHelper.storeAndLabel(
            fileBytes = bytes,
            fileName = null,
            mime = "application/pdf",
            storeBytes = { _ -> },
            storeMeta = { fn, m -> storedMeta = fn to m },
        )

        assertEquals("[file]", label)
        assertEquals(null to "application/pdf", storedMeta)
    }

    // Helper to make nullable ByteArray content-comparison usable in assertions
    private fun nullSafeContentEquals(a: ByteArray?, b: ByteArray?): Boolean =
        if (a == null && b == null) true
        else if (a == null || b == null) false
        else a.contentEquals(b)
}
