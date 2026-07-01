package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * CopyPaste-vp63.37 — unit tests for the pure helper functions extracted
 * from HistoryScreen's bulk-copy / save-file / open-file action bodies into
 * HistoryItemActions.kt.
 */
class HistoryItemActionsTest {

    private fun item(
        id: String,
        contentType: String = "text",
        isSensitive: Boolean = false,
        snippet: String = "",
    ) = ClipboardItem(
        id = id,
        contentType = contentType,
        isSensitive = isSensitive,
        wallTimeMs = 0,
        snippet = snippet,
    )

    // ── selectableTextItemsForBulkCopy ────────────────────────────────────────

    @Test
    fun `selectableTextItemsForBulkCopy keeps only selected text items`() {
        val items = listOf(
            item("a"),
            item("b"),
            item("c"),
        )
        val result = selectableTextItemsForBulkCopy(items, selectedIds = setOf("a", "c"))
        assertEquals(listOf("a", "c"), result.map { it.id })
    }

    @Test
    fun `selectableTextItemsForBulkCopy excludes sensitive items even when selected`() {
        val items = listOf(
            item("safe"),
            item("secret", isSensitive = true),
        )
        val result = selectableTextItemsForBulkCopy(items, selectedIds = setOf("safe", "secret"))
        assertEquals(listOf("safe"), result.map { it.id })
    }

    @Test
    fun `selectableTextItemsForBulkCopy excludes non-text items even when selected`() {
        val items = listOf(
            item("txt", contentType = "text"),
            item("img", contentType = "image"),
            item("file", contentType = "file"),
        )
        val result = selectableTextItemsForBulkCopy(items, selectedIds = setOf("txt", "img", "file"))
        assertEquals(listOf("txt"), result.map { it.id })
    }

    @Test
    fun `selectableTextItemsForBulkCopy preserves the input display order`() {
        val items = listOf(item("z"), item("a"), item("m"))
        val result = selectableTextItemsForBulkCopy(items, selectedIds = setOf("a", "z", "m"))
        assertEquals(listOf("z", "a", "m"), result.map { it.id })
    }

    // ── fallbackFileName ──────────────────────────────────────────────────────

    @Test
    fun `fallbackFileName returns the stored name when non-blank`() {
        assertEquals("report.pdf", fallbackFileName("report.pdf", id = "abc"))
    }

    @Test
    fun `fallbackFileName falls back to file_id-bin when name is null`() {
        assertEquals("file_abc.bin", fallbackFileName(null, id = "abc"))
    }

    @Test
    fun `fallbackFileName falls back to file_id-bin when name is blank`() {
        assertEquals("file_abc.bin", fallbackFileName("   ", id = "abc"))
    }

    // ── fileExtensionOf ───────────────────────────────────────────────────────

    @Test
    fun `fileExtensionOf lower-cases and strips the leading dot`() {
        assertEquals("pdf", fileExtensionOf("Report.PDF"))
    }

    @Test
    fun `fileExtensionOf returns empty string when there is no extension`() {
        assertEquals("", fileExtensionOf("no_extension"))
    }

    @Test
    fun `fileExtensionOf uses the LAST dot for multi-dot names`() {
        assertEquals("gz", fileExtensionOf("archive.tar.gz"))
    }
}
