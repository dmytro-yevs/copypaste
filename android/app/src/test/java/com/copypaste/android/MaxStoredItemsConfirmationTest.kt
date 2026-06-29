package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-bdac.88 / crh3.39 / crh3.108 — "Maximum stored items" cap cluster.
 *
 * ## Cross-platform semantics (the crh3.39 divergence, documented + tested)
 *
 * macOS "Max history items" is a DISPLAY filter: the daemon stores every captured
 * clip and the slider only limits how many rows the UI renders. Reducing it
 * deletes NOTHING. See crates/copypaste-ui/src/views/SettingsView/tabs/StorageTab.tsx.
 *
 * Android "Maximum stored items" is a STORED (destructive) cap: it tombstones the
 * oldest unpinned rows so the on-disk store never exceeds the cap. Reducing it
 * permanently deletes older unpinned items.
 *
 * Because the Android cap is destructive, [ClipboardRepository.planCountCapEvictions]
 * is the single source of truth for:
 *  1. the deletion count shown in the Save confirmation dialog (bdac.88), and
 *  2. the continuous count-cap enforcement run after every insert (crh3.108).
 *
 * These pure-JVM tests exercise that planner without an Android runtime.
 */
class MaxStoredItemsConfirmationTest {

    // ── bdac.88: a cap reduction computes the correct deletion count ───────────

    @Test
    fun `reducing the cap to 2 over 5 stored items plans 3 deletions`() {
        val ids = listOf("a", "b", "c", "d", "e") // oldest-first
        val evicted = ClipboardRepository.planCountCapEvictions(ids, emptySet(), maxItems = 2)
        assertEquals("5 stored, cap 2 → 3 deletions", 3, evicted.size)
        // Oldest unpinned evicted first.
        assertEquals(listOf("a", "b", "c"), evicted)
    }

    // ── bdac.88: Cancel = non-destructive (revert to old cap → zero deletions) ─

    @Test
    fun `cancel semantics — keeping the old cap deletes nothing`() {
        val ids = listOf("a", "b", "c", "d", "e")
        // The user tried cap=2 but cancelled; the persisted cap stays at (say) 5.
        val evicted = ClipboardRepository.planCountCapEvictions(ids, emptySet(), maxItems = 5)
        assertTrue("Reverting to a cap >= live count must delete nothing", evicted.isEmpty())
    }

    @Test
    fun `a cap at or above live count is a non-destructive no-op`() {
        val ids = listOf("a", "b", "c")
        assertTrue(ClipboardRepository.planCountCapEvictions(ids, emptySet(), maxItems = 3).isEmpty())
        assertTrue(ClipboardRepository.planCountCapEvictions(ids, emptySet(), maxItems = 10).isEmpty())
    }

    // ── crh3.108: continuous prune keeps size <= limit across N inserts ────────

    @Test
    fun `continuous enforcement keeps the store at or under the limit across many inserts`() {
        val limit = 10
        val store = mutableListOf<String>()
        repeat(250) { i ->
            // Each insert appends to the tail (newest), then the post-insert prune runs.
            store.add("item-$i")
            val evicted = ClipboardRepository.planCountCapEvictions(store, emptySet(), maxItems = limit)
            store.removeAll(evicted.toSet())
            assertTrue(
                "After insert #$i the store (${store.size}) must never exceed the limit $limit",
                store.size <= limit,
            )
        }
        // The survivors are the most-recent `limit` items.
        assertEquals(limit, store.size)
        assertEquals((240 until 250).map { "item-$it" }, store)
    }

    // ── pinned rows are never pruned ───────────────────────────────────────────

    @Test
    fun `pinned rows are never evicted even when they exceed the cap`() {
        val ids = listOf("pinA", "pinB", "pinC", "u1", "u2")
        val pinned = setOf("pinA", "pinB", "pinC")
        // cap 2, but 3 pinned items already exceed it → only unpinned may be evicted.
        val evicted = ClipboardRepository.planCountCapEvictions(ids, pinned, maxItems = 2)
        assertTrue("No pinned id may ever be planned for eviction", evicted.none { it in pinned })
        // Both unpinned items are evicted to shrink the live count toward the cap.
        assertEquals(setOf("u1", "u2"), evicted.toSet())
    }

    @Test
    fun `pinned rows count toward the limit but only unpinned are deleted`() {
        val ids = listOf("pinA", "u1", "u2", "u3", "u4")
        val pinned = setOf("pinA")
        // cap 3, 5 live → 2 must go, and they must be the oldest UNPINNED ones.
        val evicted = ClipboardRepository.planCountCapEvictions(ids, pinned, maxItems = 3)
        assertEquals(listOf("u1", "u2"), evicted)
    }

    // ── crh3.39: Android stored cap DELETES; macOS display filter does NOT ─────

    @Test
    fun `android stored cap is destructive — planner returns rows to delete`() {
        val ids = listOf("a", "b", "c", "d")
        val evicted = ClipboardRepository.planCountCapEvictions(ids, emptySet(), maxItems = 1)
        // Android: a reduction actually removes rows (destructive stored cap).
        // macOS parity note: a display-only filter would remove ZERO rows here.
        assertTrue("Android cap must plan real deletions (it is a stored cap)", evicted.isNotEmpty())
        assertEquals(3, evicted.size)
    }
}
