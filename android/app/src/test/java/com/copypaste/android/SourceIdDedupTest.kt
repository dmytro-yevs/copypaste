package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM tests for the LOW-2 source-id dedup predicate
 * [ClipboardRepository.isNewSourceId]. No Android/FFI deps, so these run under
 * `./gradlew testDebugUnitTest` without an emulator.
 *
 * The predicate is what stops an incoming synced item — fetched by BOTH the FGS
 * poll loop and the WorkManager worker via the shared `lastSupabasePollWallTime`
 * cursor — from being stored twice under two fresh local UUIDs.
 */
class SourceIdDedupTest {

    @Test
    fun unseenSourceIdIsNew() {
        assertTrue(ClipboardRepository.isNewSourceId("item-1", emptySet()))
        assertTrue(ClipboardRepository.isNewSourceId("item-3", setOf("item-1", "item-2")))
    }

    @Test
    fun alreadySeenSourceIdIsNotNew() {
        assertFalse(ClipboardRepository.isNewSourceId("item-1", setOf("item-1")))
        assertFalse(ClipboardRepository.isNewSourceId("item-2", setOf("item-1", "item-2")))
    }

    @Test
    fun secondFetchOfSameRowIsDeduped() {
        // Simulate the two pollers racing on the same remote row: the first store
        // records the id, the second store sees it and is rejected.
        val seen = LinkedHashSet<String>()
        val sourceId = "cloud-row-42"

        assertTrue("first fetch stores", ClipboardRepository.isNewSourceId(sourceId, seen))
        seen.add(sourceId)
        assertFalse("re-fetch is deduped", ClipboardRepository.isNewSourceId(sourceId, seen))
    }

    @Test
    fun distinctRowsAreEachStored() {
        val seen = LinkedHashSet<String>()
        listOf("a", "b", "c").forEach { id ->
            assertTrue(ClipboardRepository.isNewSourceId(id, seen))
            seen.add(id)
        }
        assertFalse(ClipboardRepository.isNewSourceId("a", seen))
        assertFalse(ClipboardRepository.isNewSourceId("b", seen))
        assertFalse(ClipboardRepository.isNewSourceId("c", seen))
    }
}
