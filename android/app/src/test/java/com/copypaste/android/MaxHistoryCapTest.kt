package com.copypaste.android

import com.copypaste.android.ui.theme.MAX_ITEMS_STEP_VALUES
import com.copypaste.android.ui.theme.MAX_ITEMS_STEP_LABELS
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-iovc: Max History Items cap — label visibility and enforcement.
 *
 * Root cause: The "Max History Items" stepped slider in Settings wrote the pref
 * but the in-store prune was only invoked on the next clipboard capture, not
 * immediately on Save. The UI label (value display in SteppedSliderRow) was
 * correctly wired via MAX_ITEMS_STEP_LABELS, so "no label" was a historical
 * note. The enforcement gap is: saving the pref must retroactively prune.
 *
 * Fix: ClipboardRepository.applyHistoryCap() (public wrapper around the
 * private pruneToLimits()) is called from SettingsActivity.persistAll() after
 * writing maxHistoryItems. These tests verify the step arrays (label/value
 * consistency) and the pruning logic (pure-JVM simulation).
 */
class MaxHistoryCapTest {

    // ── Step-array invariants ──────────────────────────────────────────────────

    @Test
    fun `MAX_ITEMS_STEP_LABELS and MAX_ITEMS_STEP_VALUES are same length`() {
        assertEquals(
            "Step values and labels must be same length",
            MAX_ITEMS_STEP_VALUES.size,
            MAX_ITEMS_STEP_LABELS.size,
        )
    }

    @Test
    fun `MAX_ITEMS_STEP_VALUES sentinel is 100000 (Unlimited)`() {
        assertEquals(
            "Last step must be the Unlimited sentinel (100 000)",
            100_000L,
            MAX_ITEMS_STEP_VALUES.last(),
        )
    }

    @Test
    fun `MAX_ITEMS_STEP_LABELS last entry is Unlimited`() {
        assertEquals(
            "Last label must be 'Unlimited'",
            "Unlimited",
            MAX_ITEMS_STEP_LABELS.last(),
        )
    }

    @Test
    fun `MAX_ITEMS_STEP_VALUES are strictly ascending`() {
        for (i in 1 until MAX_ITEMS_STEP_VALUES.size) {
            assertTrue(
                "Step values must be strictly ascending at index $i",
                MAX_ITEMS_STEP_VALUES[i] > MAX_ITEMS_STEP_VALUES[i - 1],
            )
        }
    }

    @Test
    fun `MAX_ITEMS_STEP_VALUES minimum is at least 100`() {
        assertTrue(
            "Minimum step must be ≥ 100 so the cap is not absurdly low",
            MAX_ITEMS_STEP_VALUES.first() >= 100L,
        )
    }

    // ── Pure-JVM cap enforcement simulation ───────────────────────────────────

    /**
     * Simulate the count-cap eviction logic from pruneToLimits() in pure Kotlin.
     * Items are represented as a simple [List<String>] (ids), oldest-first.
     */
    private fun simulateCountCap(ids: List<String>, pinned: Set<String>, maxItems: Int): List<String> {
        val unpinned = ids.filter { it !in pinned }.toMutableList()
        val result   = ids.toMutableList()
        while (unpinned.isNotEmpty() && result.size > maxItems) {
            val evict = unpinned.removeAt(0)
            result.remove(evict)
        }
        return result
    }

    @Test
    fun `cap of 2 keeps only 2 items when 5 are stored`() {
        val ids = listOf("a", "b", "c", "d", "e")
        val kept = simulateCountCap(ids, emptySet(), maxItems = 2)
        assertEquals(2, kept.size)
    }

    @Test
    fun `cap never evicts pinned items`() {
        val ids = listOf("pinA", "pinB", "c", "d", "e")
        val pinned = setOf("pinA", "pinB")
        val kept = simulateCountCap(ids, pinned, maxItems = 2)
        assertTrue("pinA must be kept", "pinA" in kept)
        assertTrue("pinB must be kept", "pinB" in kept)
    }

    @Test
    fun `cap of 100000 (Unlimited) evicts nothing`() {
        val ids = (1..500).map { "id$it" }
        val kept = simulateCountCap(ids, emptySet(), maxItems = 100_000)
        assertEquals("Unlimited sentinel must not evict any item", ids.size, kept.size)
    }

    @Test
    fun `cap exactly at count is a no-op`() {
        val ids = listOf("a", "b", "c")
        val kept = simulateCountCap(ids, emptySet(), maxItems = 3)
        assertEquals(3, kept.size)
    }

    @Test
    fun `eviction removes oldest first (head of unpinned list)`() {
        // Oldest items are at the front of the unpinned list.
        val ids = listOf("oldest", "middle", "newest")
        val kept = simulateCountCap(ids, emptySet(), maxItems = 1)
        assertEquals(listOf("newest"), kept)
    }
}
