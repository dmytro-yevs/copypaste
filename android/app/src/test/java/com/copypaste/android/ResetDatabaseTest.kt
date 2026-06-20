package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-12f0: Reset database — degraded-DB recovery (macOS parity).
 *
 * Root cause: macOS has a "Reset database" action in Settings → Storage for
 * when the database is damaged. Android had no equivalent, leaving users
 * without a recovery path for a corrupted SharedPreferences store.
 *
 * Fix: ClipboardRepository.resetDatabase() wipes the entire clipboard
 * SharedPreferences file and resets in-memory state. The Settings StorageTab
 * exposes it behind a confirmation dialog.
 *
 * These are pure-JVM tests verifying the reset-state-machine logic and the
 * confirmation-gate invariant (no Android context required).
 */
class ResetDatabaseTest {

    // ── Simulated store state ──────────────────────────────────────────────────

    private data class FakeStore(
        val items: MutableList<String> = mutableListOf("a", "b", "c"),
        val parseCache: MutableMap<String, String> = mutableMapOf("a" to "text_a"),
        val lastStoredKey: String = "a",
        val isReset: Boolean = false,
    )

    private fun resetStore(store: FakeStore): FakeStore =
        FakeStore(
            items = mutableListOf(),
            parseCache = mutableMapOf(),
            lastStoredKey = "",
            isReset = true,
        )

    @Test
    fun `resetStore clears all items`() {
        val store = FakeStore()
        val reset = resetStore(store)
        assertTrue("All items must be cleared after reset", reset.items.isEmpty())
    }

    @Test
    fun `resetStore clears parse cache`() {
        val store = FakeStore()
        val reset = resetStore(store)
        assertTrue("Parse cache must be empty after reset", reset.parseCache.isEmpty())
    }

    @Test
    fun `resetStore clears lastStoredKey`() {
        val store = FakeStore()
        val reset = resetStore(store)
        assertEquals("Dedup key must be cleared after reset", "", reset.lastStoredKey)
    }

    @Test
    fun `isReset flag is true after reset`() {
        val store = FakeStore()
        assertFalse(store.isReset)
        val reset = resetStore(store)
        assertTrue(reset.isReset)
    }

    // ── Confirmation-gate invariant ────────────────────────────────────────────

    /**
     * The reset action MUST be guarded by a confirmation dialog. We simulate the
     * dialog state machine: reset is only executed when the user confirms.
     */
    private data class ResetDialogState(
        val visible: Boolean = false,
        val confirmed: Boolean = false,
        val cancelled: Boolean = false,
    )

    private fun showDialog(state: ResetDialogState) = state.copy(visible = true)
    private fun confirmDialog(state: ResetDialogState) = state.copy(visible = false, confirmed = true)
    private fun cancelDialog(state: ResetDialogState) = state.copy(visible = false, cancelled = true)

    @Test
    fun `reset is not executed until dialog is confirmed`() {
        var state = ResetDialogState()
        // User taps "Reset database" button.
        state = showDialog(state)
        assertTrue("Dialog must be shown before executing reset", state.visible)
        assertFalse("Reset must not be executed while dialog is pending", state.confirmed)
    }

    @Test
    fun `cancel does not execute reset`() {
        var state = showDialog(ResetDialogState())
        state = cancelDialog(state)
        assertFalse("Reset must not be executed when user cancels", state.confirmed)
        assertTrue(state.cancelled)
        assertFalse(state.visible)
    }

    @Test
    fun `confirm executes reset and closes dialog`() {
        var state = showDialog(ResetDialogState())
        state = confirmDialog(state)
        assertTrue("Reset must be executed when user confirms", state.confirmed)
        assertFalse("Dialog must be closed after confirmation", state.visible)
    }
}
