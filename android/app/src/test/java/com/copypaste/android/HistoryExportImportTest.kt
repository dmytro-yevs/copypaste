package com.copypaste.android

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Unit tests for CopyPaste-8jx8: clipboard history export/import.
 *
 * The export format is a JSON array of objects:
 *   { "version": 1, "exported_at": <epochMs>, "items": [ ... ] }
 *
 * Each item entry:
 *   { "id": "uuid", "content_type": "text", "snippet": "preview", "wall_time_ms": 12345 }
 *
 * Design decisions:
 *   - Only TEXT items are exported (images/files contain binary data that is too large
 *     and too opaque for a portable JSON export format).
 *   - The export is PLAINTEXT (decrypted snippets only — the full encrypted blobs are NOT
 *     portable across devices/keys). Import creates NEW items on the target device
 *     encrypted with the target device's key.
 *   - Sensitive items are skipped on export (isSensitive == true) to avoid leaking
 *     passwords/tokens into unencrypted export files.
 *   - Pinned state is preserved in the export so users can round-trip their pinned clips.
 *
 * These tests validate the JSON structure and round-trip invariants without
 * Android runtime dependencies.
 */
class HistoryExportImportTest {

    // ── Export format ─────────────────────────────────────────────────────────

    data class ExportItem(
        val id: String,
        val contentType: String,
        val snippet: String,
        val wallTimeMs: Long,
        val pinned: Boolean,
    )

    private fun buildExportJson(items: List<ExportItem>, exportedAt: Long): String {
        val arr = JSONArray()
        for (item in items) {
            val obj = JSONObject()
            obj.put("id", item.id)
            obj.put("content_type", item.contentType)
            obj.put("snippet", item.snippet)
            obj.put("wall_time_ms", item.wallTimeMs)
            obj.put("pinned", item.pinned)
            arr.put(obj)
        }
        val root = JSONObject()
        root.put("version", 1)
        root.put("exported_at", exportedAt)
        root.put("items", arr)
        return root.toString()
    }

    private fun parseExportJson(json: String): Pair<Long, List<ExportItem>> {
        val root = JSONObject(json)
        assertEquals("Unsupported export version", 1, root.getInt("version"))
        val exportedAt = root.getLong("exported_at")
        val arr = root.getJSONArray("items")
        val items = (0 until arr.length()).map { i ->
            val obj = arr.getJSONObject(i)
            ExportItem(
                id = obj.getString("id"),
                contentType = obj.getString("content_type"),
                snippet = obj.getString("snippet"),
                wallTimeMs = obj.getLong("wall_time_ms"),
                pinned = obj.optBoolean("pinned", false),
            )
        }
        return exportedAt to items
    }

    @Test
    fun exportJson_version_isOne() {
        val json = buildExportJson(emptyList(), 1_000L)
        val root = JSONObject(json)
        assertEquals(1, root.getInt("version"))
    }

    @Test
    fun exportJson_exportedAt_isPreserved() {
        val ts = 1_718_000_000_000L
        val json = buildExportJson(emptyList(), ts)
        val root = JSONObject(json)
        assertEquals(ts, root.getLong("exported_at"))
    }

    @Test
    fun exportJson_emptyItems_producesEmptyArray() {
        val json = buildExportJson(emptyList(), 0L)
        val (_, items) = parseExportJson(json)
        assertEquals(0, items.size)
    }

    @Test
    fun exportJson_roundTrip_singleItem() {
        val item = ExportItem(
            id = "abc-123",
            contentType = "text",
            snippet = "Hello, world!",
            wallTimeMs = 12345L,
            pinned = true,
        )
        val json = buildExportJson(listOf(item), 9999L)
        val (_, items) = parseExportJson(json)
        assertEquals(1, items.size)
        assertEquals(item.id, items[0].id)
        assertEquals(item.contentType, items[0].contentType)
        assertEquals(item.snippet, items[0].snippet)
        assertEquals(item.wallTimeMs, items[0].wallTimeMs)
        assertEquals(item.pinned, items[0].pinned)
    }

    @Test
    fun exportJson_roundTrip_multipleItems() {
        val items = (1..5).map { i ->
            ExportItem(
                id = "id-$i",
                contentType = "text",
                snippet = "Item $i",
                wallTimeMs = 1000L * i,
                pinned = i == 1,
            )
        }
        val json = buildExportJson(items, 0L)
        val (_, parsed) = parseExportJson(json)
        assertEquals(5, parsed.size)
        for (i in items.indices) {
            assertEquals(items[i].id, parsed[i].id)
            assertEquals(items[i].snippet, parsed[i].snippet)
            assertEquals(items[i].pinned, parsed[i].pinned)
        }
    }

    // ── Export filter: only text items ────────────────────────────────────────

    @Test
    fun onlyTextItems_areExported() {
        // The export function must filter out image and file items.
        val mixed = listOf(
            ExportItem("t1", "text", "Hello", 1L, false),
            ExportItem("i1", "image", "[image]", 2L, false),
            ExportItem("f1", "file", "[file: doc.pdf]", 3L, false),
            ExportItem("t2", "text", "World", 4L, false),
        )
        val exported = mixed.filter { it.contentType == "text" }
        assertEquals(2, exported.size)
        assertTrue(exported.all { it.contentType == "text" })
        assertFalse(exported.any { it.contentType == "image" })
        assertFalse(exported.any { it.contentType == "file" })
    }

    // ── Import deduplication ──────────────────────────────────────────────────

    @Test
    fun importDedup_existingId_isSkipped() {
        // If an item with the same ID already exists locally, import must skip it
        // to avoid duplicates (ID is the cross-device stable identity).
        val existingIds = setOf("existing-1", "existing-2")
        val incoming = listOf(
            ExportItem("existing-1", "text", "Old text", 1L, false),
            ExportItem("new-id-3", "text", "New text", 2L, false),
        )
        val toImport = incoming.filter { it.id !in existingIds }
        assertEquals(1, toImport.size)
        assertEquals("new-id-3", toImport[0].id)
    }

    @Test
    fun importDedup_allNew_importsAll() {
        val existingIds = emptySet<String>()
        val incoming = listOf(
            ExportItem("a", "text", "A", 1L, false),
            ExportItem("b", "text", "B", 2L, false),
        )
        val toImport = incoming.filter { it.id !in existingIds }
        assertEquals(2, toImport.size)
    }

    // ── Pinned state preservation ─────────────────────────────────────────────

    @Test
    fun pinnedItems_areFlaggedInExport() {
        val pinned = ExportItem("p1", "text", "Pinned!", 1L, true)
        val json = buildExportJson(listOf(pinned), 0L)
        val (_, items) = parseExportJson(json)
        assertTrue("Pinned flag must survive round-trip", items[0].pinned)
    }

    @Test
    fun unpinnedItems_haveDefaultFalseOnImport() {
        // An export that omits the 'pinned' field must default to false on import.
        val json = """{"version":1,"exported_at":0,"items":[{"id":"x","content_type":"text","snippet":"hi","wall_time_ms":1}]}"""
        val (_, items) = parseExportJson(json)
        assertFalse("Missing 'pinned' field must default to false", items[0].pinned)
    }

    // ── Sensitive-items filter (CopyPaste-crh3.40) ────────────────────────────
    //
    // The export filter logic mirrors ClipboardRepository.exportHistory():
    //   isText && (!isSensitive || includeSensitive)
    //
    // Tests here validate that logic as a pure function so they can run on the
    // JVM without any Android context (no SharedPreferences / FFI needed).

    /** Represents a raw history item before export filtering. */
    private data class RawHistoryItem(
        val id: String,
        val contentType: String,
        val isSensitive: Boolean,
        val snippet: String,
    )

    /**
     * Pure filter that mirrors the loop body in ClipboardRepository.exportHistory().
     * Keep in sync with the production implementation.
     */
    private fun filterForExport(
        items: List<RawHistoryItem>,
        includeSensitive: Boolean,
    ): List<RawHistoryItem> = items.filter { item ->
        item.contentType == "text" && (!item.isSensitive || includeSensitive)
    }

    @Test
    fun sensitiveItems_areExcluded_byDefault() {
        // Default (includeSensitive = false): sensitive text items must be omitted.
        val items = listOf(
            RawHistoryItem("s1", "text", isSensitive = true,  snippet = "secret"),
            RawHistoryItem("n1", "text", isSensitive = false, snippet = "normal"),
        )
        val exported = filterForExport(items, includeSensitive = false)
        assertEquals("Only non-sensitive item should be exported", 1, exported.size)
        assertEquals("n1", exported[0].id)
    }

    @Test
    fun sensitiveItems_areIncluded_whenFlagIsTrue() {
        // includeSensitive = true: all text items (sensitive + non-sensitive) must appear.
        val items = listOf(
            RawHistoryItem("s1", "text", isSensitive = true,  snippet = "secret"),
            RawHistoryItem("n1", "text", isSensitive = false, snippet = "normal"),
        )
        val exported = filterForExport(items, includeSensitive = true)
        assertEquals("Both items should be exported when flag is true", 2, exported.size)
        assertTrue("Sensitive item present", exported.any { it.id == "s1" })
        assertTrue("Normal item present",    exported.any { it.id == "n1" })
    }

    @Test
    fun sensitiveFilter_allSensitive_excludedByDefault() {
        val items = listOf(
            RawHistoryItem("s1", "text", isSensitive = true, snippet = "pw1"),
            RawHistoryItem("s2", "text", isSensitive = true, snippet = "pw2"),
        )
        assertEquals(0, filterForExport(items, includeSensitive = false).size)
    }

    @Test
    fun sensitiveFilter_allSensitive_includedWhenFlagIsTrue() {
        val items = listOf(
            RawHistoryItem("s1", "text", isSensitive = true, snippet = "pw1"),
            RawHistoryItem("s2", "text", isSensitive = true, snippet = "pw2"),
        )
        assertEquals(2, filterForExport(items, includeSensitive = true).size)
    }

    @Test
    fun sensitiveFilter_nonTextItems_alwaysExcluded() {
        // Image / file items with isSensitive=false must still be excluded from export.
        val items = listOf(
            RawHistoryItem("i1", "image", isSensitive = false, snippet = "[img]"),
            RawHistoryItem("f1", "file",  isSensitive = false, snippet = "[file]"),
            RawHistoryItem("t1", "text",  isSensitive = false, snippet = "hello"),
        )
        val exported = filterForExport(items, includeSensitive = true)
        assertEquals("Only the text item should survive", 1, exported.size)
        assertEquals("t1", exported[0].id)
    }

    @Test
    fun sensitiveFilter_mixedList_correctCountBothModes() {
        // 2 sensitive text, 2 non-sensitive text, 1 sensitive image, 1 non-sensitive file
        val items = listOf(
            RawHistoryItem("st1", "text",  isSensitive = true,  snippet = "s1"),
            RawHistoryItem("st2", "text",  isSensitive = true,  snippet = "s2"),
            RawHistoryItem("nt1", "text",  isSensitive = false, snippet = "n1"),
            RawHistoryItem("nt2", "text",  isSensitive = false, snippet = "n2"),
            RawHistoryItem("si1", "image", isSensitive = true,  snippet = "[img]"),
            RawHistoryItem("nf1", "file",  isSensitive = false, snippet = "[file]"),
        )
        // Default (off): only 2 non-sensitive text items
        assertEquals(2, filterForExport(items, includeSensitive = false).size)
        // On: 4 text items (2 sensitive + 2 non-sensitive); images/files still excluded
        assertEquals(4, filterForExport(items, includeSensitive = true).size)
    }
}
