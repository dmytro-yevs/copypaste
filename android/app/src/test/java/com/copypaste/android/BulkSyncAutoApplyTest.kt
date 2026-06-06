package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * Unit tests for the bulk-sync auto-apply selection logic.
 *
 * When N text clips arrive in a single catch-up batch, the app must NOT write
 * each one to the system clipboard — only the NEWEST (highest wall_time, or
 * the last one in arrival order when wall_times are equal) should be applied
 * once after the whole drain completes.
 *
 * The [FgsSyncLoop.Companion.newestTextClip] helper is a pure function
 * (no Android runtime, no coroutines) and can therefore be tested on the JVM.
 */
class BulkSyncAutoApplyTest {

    @Test
    fun noTextItems_returnsNull() {
        val result = FgsSyncLoop.newestTextClip(emptyList())
        assertNull(result)
    }

    @Test
    fun singleTextItem_returnsThatItem() {
        val clips = listOf("hello" to 1000L)
        val result = FgsSyncLoop.newestTextClip(clips)
        assertEquals("hello", result)
    }

    @Test
    fun multipleItems_returnsHighestWallTime() {
        val clips = listOf(
            "first" to 1000L,
            "second" to 3000L,
            "third" to 2000L,
        )
        val result = FgsSyncLoop.newestTextClip(clips)
        assertEquals("second", result)
    }

    @Test
    fun tieOnWallTime_returnsLastArrival() {
        // When wall_time is the same (e.g. all arrived at the same millisecond),
        // pick the last one in processing order (latest in the batch row order).
        val clips = listOf(
            "alpha" to 5000L,
            "beta" to 5000L,
            "gamma" to 5000L,
        )
        val result = FgsSyncLoop.newestTextClip(clips)
        assertEquals("gamma", result)
    }

    @Test
    fun newerItemAtHead_stillReturnsTrueNewest() {
        val clips = listOf(
            "newest" to 9999L,
            "older1" to 1000L,
            "older2" to 500L,
        )
        val result = FgsSyncLoop.newestTextClip(clips)
        assertEquals("newest", result)
    }
}
