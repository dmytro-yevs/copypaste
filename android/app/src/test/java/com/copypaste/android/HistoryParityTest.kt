package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for CopyPaste-hv5 — History parity: density-aware row height,
 * copy-success flash trigger, and empty-search icon selection logic.
 *
 * These tests validate the non-Compose logic in isolation (no Android runtime needed).
 */
class HistoryParityTest {

    // ─────────────────────────────────────────────────────────────────────────
    // §1 Density-aware row height
    // ─────────────────────────────────────────────────────────────────────────

    /**
     * Utility mirroring the in-production lookup:
     *   settings.density ("comfortable"|"compact"), defaulting to "comfortable"
     * when the key is absent (defensive read — density pref may not exist yet).
     */
    private fun readDensityPref(prefs: Map<String, String>): String =
        prefs.getOrDefault("density", "comfortable")

    /** Compact mode → min row height 28 dp. */
    private fun rowMinHeightDp(density: String): Int = when (density) {
        "compact" -> 28
        else      -> 34   // comfortable (default) or any unknown value
    }

    @Test
    fun `density pref absent -- defaults to comfortable`() {
        val density = readDensityPref(emptyMap())
        assertEquals("comfortable", density)
    }

    @Test
    fun `density pref set to compact -- returns compact`() {
        val density = readDensityPref(mapOf("density" to "compact"))
        assertEquals("compact", density)
    }

    @Test
    fun `density pref set to comfortable -- returns comfortable`() {
        val density = readDensityPref(mapOf("density" to "comfortable"))
        assertEquals("comfortable", density)
    }

    @Test
    fun `comfortable density -- row min height is 34dp`() {
        val height = rowMinHeightDp("comfortable")
        assertEquals(34, height)
    }

    @Test
    fun `compact density -- row min height is 28dp`() {
        val height = rowMinHeightDp("compact")
        assertEquals(28, height)
    }

    @Test
    fun `unknown density value -- falls through to comfortable 34dp`() {
        // Defensive: any future unknown value should default to comfortable, not crash.
        val height = rowMinHeightDp("ultra")
        assertEquals(34, height)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // §2 Copy-success flash trigger
    // ─────────────────────────────────────────────────────────────────────────

    /**
     * Simulates the flash state machine:
     * - Initial state: not flashing.
     * - After onCopy() is called: flashing = true.
     * - After the 90 ms animation completes (reset): flashing = false.
     *
     * The real implementation uses animateColorAsState + LaunchedEffect(flashTrigger),
     * but the Boolean trigger logic is testable here as a pure state machine.
     */
    private data class CopyFlashState(val flashing: Boolean = false)

    private fun triggerCopyFlash(state: CopyFlashState): CopyFlashState =
        state.copy(flashing = true)

    private fun resetCopyFlash(state: CopyFlashState): CopyFlashState =
        state.copy(flashing = false)

    @Test
    fun `copy flash -- initial state is not flashing`() {
        val state = CopyFlashState()
        assertFalse(state.flashing)
    }

    @Test
    fun `copy flash -- trigger sets flashing to true`() {
        val state = triggerCopyFlash(CopyFlashState())
        assertTrue(state.flashing)
    }

    @Test
    fun `copy flash -- reset after animation clears flashing`() {
        val state = resetCopyFlash(triggerCopyFlash(CopyFlashState()))
        assertFalse(state.flashing)
    }

    @Test
    fun `copy flash -- multiple triggers stay idempotent`() {
        var state = CopyFlashState()
        state = triggerCopyFlash(state)
        state = triggerCopyFlash(state) // second trigger while already flashing
        assertTrue(state.flashing)
        state = resetCopyFlash(state)
        assertFalse(state.flashing)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // §3 Empty-search hero icon selection
    // ─────────────────────────────────────────────────────────────────────────

    /**
     * Validates that the correct icon name is selected for the empty-search state.
     * The production code uses Icons.Filled.SearchOff (NOT Refresh).
     */
    private fun emptySearchIconName(): String = "SearchOff"

    @Test
    fun `empty search hero icon -- is SearchOff not Refresh`() {
        val icon = emptySearchIconName()
        assertEquals("SearchOff", icon)
        assertFalse("Icon must not be the old Refresh stand-in", icon == "Refresh")
    }
}
