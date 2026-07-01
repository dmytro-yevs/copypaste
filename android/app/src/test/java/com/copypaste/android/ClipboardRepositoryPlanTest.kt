package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-ra15.4: Verifies that the top-level planner/LWW helper functions extracted
 * from [ClipboardRepository]'s companion object into [ClipboardRepositoryPlan.kt] behave
 * identically to the companion-object delegation stubs.
 *
 * These tests call the TOP-LEVEL functions directly (unqualified). Before extraction the
 * top-level functions do not exist → compile error → "failing test" step. After creating
 * [ClipboardRepositoryPlan.kt] the tests compile and pass, while the companion stubs
 * continue to delegate to the same implementations.
 */
class ClipboardRepositoryPlanTest {

    // ── planCountCapEvictions (top-level) ──────────────────────────────────────

    @Test
    fun `planCountCapEvictions top-level matches companion result for basic cap reduction`() {
        val ids = listOf("a", "b", "c", "d", "e") // oldest-first
        // Unqualified call → top-level function from ClipboardRepositoryPlan.kt
        val evicted = planCountCapEvictions(ids, emptySet(), maxItems = 2)
        // Same result as ClipboardRepository.planCountCapEvictions (companion delegation)
        val evictedViaCompanion = ClipboardRepository.planCountCapEvictions(ids, emptySet(), maxItems = 2)
        assertEquals("top-level and companion must return identical results", evictedViaCompanion, evicted)
        assertEquals(listOf("a", "b", "c"), evicted)
    }

    @Test
    fun `planCountCapEvictions pinned rows are never evicted`() {
        val ids = listOf("p1", "p2", "u1", "u2", "u3")
        val pinned = setOf("p1", "p2")
        val evicted = planCountCapEvictions(ids, pinned, maxItems = 2)
        assertTrue("Pinned ids must never be evicted", evicted.none { it in pinned })
        assertEquals(setOf("u1", "u2", "u3"), evicted.toSet())
    }

    @Test
    fun `planCountCapEvictions cap above live count returns empty`() {
        val ids = listOf("a", "b", "c")
        assertTrue(planCountCapEvictions(ids, emptySet(), maxItems = 5).isEmpty())
    }

    @Test
    fun `planCountCapEvictions cap of zero is coerced to 1`() {
        // coerceAtLeast(1): a persisted cap of 0 must not wipe all items
        val ids = listOf("a", "b", "c")
        val evicted = planCountCapEvictions(ids, emptySet(), maxItems = 0)
        assertEquals("cap 0 treated as 1 — only oldest 2 evicted", 2, evicted.size)
    }

    // ── nextLamportTs (top-level) ──────────────────────────────────────────────

    @Test
    fun `nextLamportTs returns prevLamport+1 when it exceeds nowMs`() {
        // prevLamport+1 = 11 > nowMs = 5 → 11
        assertEquals(11L, nextLamportTs(10L, 5L))
    }

    @Test
    fun `nextLamportTs returns nowMs when it exceeds prevLamport+1`() {
        // prevLamport+1 = 6 < nowMs = 100 → 100
        assertEquals(100L, nextLamportTs(5L, 100L))
    }

    @Test
    fun `nextLamportTs returns prevLamport+1 on tie`() {
        // prevLamport+1 = 101, nowMs = 100 → 101
        assertEquals(101L, nextLamportTs(100L, 100L))
    }

    @Test
    fun `nextLamportTs top-level matches companion result`() {
        val prev = 42L
        val now = 500L
        assertEquals(
            "top-level and companion must return identical results",
            ClipboardRepository.nextLamportTs(prev, now),
            nextLamportTs(prev, now),
        )
    }
}
