package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Unit tests for the bounded LRU backing [AppIconHelper.newIconCache].
 *
 * Regression guard for CopyPaste-d4iq: the icon cache used to be an unbounded
 * [java.util.concurrent.ConcurrentHashMap] that grew one entry per distinct source
 * package for the whole process lifetime. It is now an access-ordered LRU capped at
 * [AppIconHelper.MAX_CACHE_ENTRIES], mirroring the web client's LRU-128.
 *
 * The cache is exercised directly (no [android.content.Context]) so these run on the
 * plain JVM test runner with no Robolectric/mock dependency.
 */
class AppIconCacheTest {

    @Test fun capDefaultsTo128() {
        assertEquals(128, AppIconHelper.MAX_CACHE_ENTRIES)
    }

    @Test fun neverExceedsCap() {
        val cap = 8
        val cache = AppIconHelper.newIconCache(cap)
        // Insert far more than the cap.
        for (i in 0 until cap * 10) {
            cache["pkg.$i"] = "icon-$i"
            assertTrue("size must stay <= cap", cache.size <= cap)
        }
        assertEquals(cap, cache.size)
    }

    @Test fun evictsLeastRecentlyUsed() {
        val cache = AppIconHelper.newIconCache(maxEntries = 3)
        cache["a"] = "1"
        cache["b"] = "2"
        cache["c"] = "3"
        // Touch "a" so it becomes most-recently-used; "b" is now the LRU.
        assertEquals("1", cache["a"])
        // Inserting a 4th entry must evict "b" (the LRU), not "a".
        cache["d"] = "4"

        assertEquals(3, cache.size)
        assertTrue("most-recently-used 'a' must survive", cache.containsKey("a"))
        assertFalse("least-recently-used 'b' must be evicted", cache.containsKey("b"))
        assertTrue(cache.containsKey("c"))
        assertTrue(cache.containsKey("d"))
    }

    @Test fun reinsertRefreshesRecency() {
        val cache = AppIconHelper.newIconCache(maxEntries = 2)
        cache["a"] = "1"
        cache["b"] = "2"
        // Re-write "a" -> becomes most-recently-used; "b" is the LRU.
        cache["a"] = "1b"
        cache["c"] = "3" // evicts "b"

        assertEquals(2, cache.size)
        assertEquals("1b", cache["a"])
        assertNull(cache["b"])
        assertEquals("3", cache["c"])
    }

    @Test fun emptyStringSentinelIsAValidValue() {
        // ABSENT is stored as "" — confirm the LRU treats it like any other value
        // and still counts toward the cap.
        val cache = AppIconHelper.newIconCache(maxEntries = 2)
        cache["x"] = ""
        cache["y"] = ""
        cache["z"] = ""
        assertEquals(2, cache.size)
        assertFalse(cache.containsKey("x")) // evicted
    }

    @Test fun clearCacheEmptiesIt() {
        AppIconHelper.clearCache()
        assertEquals(0, AppIconHelper.cacheSize())
    }
}
