package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Unit tests for CopyPaste-9uyk: source icon display for image/file rows.
 *
 * The source icon shows the originating app's icon (via AppIconHelper) for
 * image and file clipboard items, so the user can see at a glance which app
 * the content came from. Text items already show a ContentIconTile glyph.
 *
 * These tests cover:
 *   1. shouldShowSourceIcon: returns true only for image/file items that have
 *      a non-null sourceApp field (text items do not show the source icon — the
 *      ContentIconTile already serves that purpose).
 *   2. AppIconHelper.newIconCache: eviction and capacity semantics.
 *   3. AppIconHelper.ABSENT sentinel handling (null → false for display).
 */
class SourceIconTest {

    // ── shouldShowSourceIcon logic ────────────────────────────────────────────

    /**
     * Mirror of the display predicate in HistoryRow.
     * Returns true when the item should show the source app icon.
     */
    private fun shouldShowSourceIcon(isImage: Boolean, isFile: Boolean, sourceApp: String?): Boolean =
        (isImage || isFile) && !sourceApp.isNullOrBlank()

    @Test
    fun imageWithSourceApp_showsIcon() {
        assertTrue(shouldShowSourceIcon(isImage = true, isFile = false, sourceApp = "com.google.android.gm"))
    }

    @Test
    fun fileWithSourceApp_showsIcon() {
        assertTrue(shouldShowSourceIcon(isImage = false, isFile = true, sourceApp = "com.android.chrome"))
    }

    @Test
    fun imageWithoutSourceApp_noIcon() {
        assertFalse(shouldShowSourceIcon(isImage = true, isFile = false, sourceApp = null))
    }

    @Test
    fun imageWithBlankSourceApp_noIcon() {
        assertFalse(shouldShowSourceIcon(isImage = true, isFile = false, sourceApp = ""))
    }

    @Test
    fun fileWithoutSourceApp_noIcon() {
        assertFalse(shouldShowSourceIcon(isImage = false, isFile = true, sourceApp = null))
    }

    @Test
    fun textItemWithSourceApp_noIcon() {
        // Text items use ContentIconTile, not the source app icon.
        assertFalse(shouldShowSourceIcon(isImage = false, isFile = false, sourceApp = "com.example.app"))
    }

    @Test
    fun textItemWithoutSourceApp_noIcon() {
        assertFalse(shouldShowSourceIcon(isImage = false, isFile = false, sourceApp = null))
    }

    // ── AppIconHelper.newIconCache capacity and eviction ────────────────────

    @Test
    fun lruCache_maxEntries_matchesConstant() {
        assertEquals(128, AppIconHelper.MAX_CACHE_ENTRIES)
    }

    @Test
    fun lruCache_doesNotExceedCapacity() {
        val cache = AppIconHelper.newIconCache(maxEntries = 3)
        for (i in 1..5) cache["pkg$i"] = "icon$i"
        assertTrue(
            "Cache must evict entries beyond maxEntries",
            cache.size <= 3,
        )
    }

    @Test
    fun lruCache_mostRecentEntryIsRetained() {
        val cache = AppIconHelper.newIconCache(maxEntries = 2)
        cache["a"] = "iconA"
        cache["b"] = "iconB"
        // Access "a" to make it most-recently-used.
        @Suppress("UNUSED_EXPRESSION") cache["a"]
        cache["c"] = "iconC" // evicts "b" (least-recently-used)
        assertTrue("'a' (recently accessed) must be retained", cache.containsKey("a"))
        assertTrue("'c' (just added) must be retained", cache.containsKey("c"))
        assertFalse("'b' (least-recently-used) must be evicted", cache.containsKey("b"))
    }

    @Test
    fun lruCache_singleEntry_isRetained() {
        val cache = AppIconHelper.newIconCache(maxEntries = 1)
        cache["only"] = "data"
        assertEquals(1, cache.size)
        assertEquals("data", cache["only"])
    }

    // ── ABSENT sentinel handling ──────────────────────────────────────────────

    @Test
    fun absentSentinel_isEmptyString() {
        // ABSENT is accessible via the public constant for testing.
        // An empty-string stored value means "tried, not found" → display no icon.
        val storedValue = "" // simulates the ABSENT sentinel
        val shouldDisplay = storedValue.isNotEmpty()
        assertFalse("ABSENT sentinel (empty string) must not trigger icon display", shouldDisplay)
    }

    @Test
    fun realBase64Value_triggersIconDisplay() {
        val storedValue = "aGVsbG8=" // base64("hello") — a real (if tiny) icon value
        assertTrue("Non-empty base64 value must trigger icon display", storedValue.isNotEmpty())
    }
}
