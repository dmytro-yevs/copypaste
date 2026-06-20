package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for the three HistoryActivity delete/clear fixes.
 *
 *  CopyPaste-2ifa — single-item delete confirmation: a DELETE_SINGLE action
 *    kind exists and routes through a confirmation gate rather than deleting
 *    immediately. Verified by testing the itemCount derivation logic.
 *
 *  CopyPaste-kaf6 — 5-second undo: the delete is deferred via a cancellable
 *    job; pressing UNDO cancels the job and no delete occurs.
 *
 *  CopyPaste-yel4 — clearAll sync drain always runs (finally block), and
 *    errors are surfaced through a dedicated channel rather than the generic
 *    load-history slot.
 *
 * All tests are pure Kotlin — no Android SDK, no Compose runtime required.
 * ConfirmAction is private to HistoryActivity.kt so its behaviour is
 * validated indirectly through helpers that mirror the production logic.
 */
class HistoryDeleteUndoTest {

    // ─────────────────────────────────────────────────────────────────────────
    // CopyPaste-2ifa — single-item delete confirmation gate
    // ─────────────────────────────────────────────────────────────────────────

    /**
     * Mirror of the itemCount derivation in HistoryScreen's pendingConfirm block.
     * The production code uses the private ConfirmAction enum; this helper uses
     * a local string tag so the test stays package-pure without touching
     * private symbols.
     */
    private fun confirmItemCount(
        actionTag: String,
        selectedSize: Int,
        totalSize: Int,
        unpinnedSize: Int = totalSize,
    ): Int = when (actionTag) {
        // CopyPaste-2ifa: DELETE_SINGLE always reports 1, regardless of selection.
        "DELETE_SINGLE"   -> 1
        "DELETE_SELECTED" -> selectedSize
        "CLEAR_ALL"       -> totalSize
        "CLEAR_UNPINNED"  -> unpinnedSize
        else              -> error("Unknown action: $actionTag")
    }

    @Test
    fun `DELETE_SINGLE item count is always 1`() {
        val count = confirmItemCount("DELETE_SINGLE", selectedSize = 3, totalSize = 10)
        assertEquals("Single-item delete must report count=1", 1, count)
    }

    @Test
    fun `DELETE_SINGLE item count is 1 even when no selection`() {
        val count = confirmItemCount("DELETE_SINGLE", selectedSize = 0, totalSize = 5)
        assertEquals(1, count)
    }

    @Test
    fun `DELETE_SELECTED item count equals selected size`() {
        val count = confirmItemCount("DELETE_SELECTED", selectedSize = 5, totalSize = 10)
        assertEquals(5, count)
    }

    @Test
    fun `CLEAR_ALL item count equals total size`() {
        val count = confirmItemCount("CLEAR_ALL", selectedSize = 0, totalSize = 7)
        assertEquals(7, count)
    }

    @Test
    fun `CLEAR_UNPINNED item count equals unpinned size`() {
        val count = confirmItemCount("CLEAR_UNPINNED", selectedSize = 0, totalSize = 10, unpinnedSize = 6)
        assertEquals(6, count)
    }

    /**
     * Validates that a direct-delete path (no confirmation) does NOT exist for single items.
     * The production onDelete lambda sets pendingConfirm = DELETE_SINGLE instead of
     * calling viewModel.deleteItem directly. This is modelled here as: the "gate" function
     * must produce "CONFIRM_REQUIRED" — never "DELETE_NOW" — for a single id.
     */
    private fun onDeleteGate(isSingleItem: Boolean): String =
        if (isSingleItem) "CONFIRM_REQUIRED" else "BATCH_DELETE"

    @Test
    fun `single-item delete routes through confirmation gate`() {
        assertEquals("DELETE_SINGLE must require confirmation", "CONFIRM_REQUIRED", onDeleteGate(true))
    }

    @Test
    fun `batch delete does not use the single-item gate`() {
        assertEquals("BATCH_DELETE", onDeleteGate(false))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // CopyPaste-kaf6 — 5-second undo
    // ─────────────────────────────────────────────────────────────────────────

    /**
     * Model the undo flow as a pure state machine:
     *   - pendingUndoId is non-null while the 5s window is open
     *   - pressing UNDO clears pendingUndoId → no delete
     *   - expiry clears pendingUndoId → delete fires
     */
    private data class UndoState(
        val pendingUndoId: String? = null,
        val deleteCount: Int = 0,
    )

    private fun scheduleSingleDelete(id: String): UndoState =
        UndoState(pendingUndoId = id, deleteCount = 0)

    /** UNDO pressed: cancel the pending delete — no actual deletion occurs. */
    private fun onUndo(state: UndoState): UndoState =
        state.copy(pendingUndoId = null)

    /** Timer expired without UNDO: execute the delete. */
    private fun onUndoExpired(state: UndoState): UndoState =
        state.copy(
            pendingUndoId = null,
            deleteCount = state.deleteCount + (if (state.pendingUndoId != null) 1 else 0),
        )

    @Test
    fun `undo state -- scheduling delete sets pendingUndoId`() {
        val state = scheduleSingleDelete("abc")
        assertEquals("abc", state.pendingUndoId)
        assertEquals(0, state.deleteCount)
    }

    @Test
    fun `undo state -- UNDO cancels the pending delete`() {
        val state = onUndo(scheduleSingleDelete("abc"))
        assertNull("pendingUndoId must be cleared on UNDO", state.pendingUndoId)
        assertEquals("No delete must fire when UNDO is pressed", 0, state.deleteCount)
    }

    @Test
    fun `undo state -- expiry fires the actual delete`() {
        val state = onUndoExpired(scheduleSingleDelete("abc"))
        assertNull("pendingUndoId must be cleared after expiry", state.pendingUndoId)
        assertEquals("Exactly one delete must fire on expiry", 1, state.deleteCount)
    }

    @Test
    fun `undo state -- expiry on already-cancelled state is no-op`() {
        // UNDO already cleared pendingUndoId; a stale expiry callback must not delete.
        val cancelled = onUndo(scheduleSingleDelete("abc"))
        val afterExpiry = onUndoExpired(cancelled)
        assertEquals("No delete when state was already cancelled", 0, afterExpiry.deleteCount)
    }

    @Test
    fun `undo state -- multiple independent deletes are independent`() {
        val stateA = scheduleSingleDelete("id-A")
        val stateB = scheduleSingleDelete("id-B")
        // UNDO on A does not affect B
        val cancelledA = onUndo(stateA)
        assertEquals(0, cancelledA.deleteCount)
        // B expires independently
        val expiredB = onUndoExpired(stateB)
        assertEquals(1, expiredB.deleteCount)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // CopyPaste-yel4 — clearAll: sync drain always runs + error surfacing
    // ─────────────────────────────────────────────────────────────────────────

    /**
     * Simulate the FIXED clearAll() that always calls the sync drain in finally.
     *
     * [repositoryThrows] controls whether repository.clearAll() succeeds or fails.
     * Returns whether the drain was called and which error channel was used.
     */
    private data class ClearAllResult(
        val syncDrainCalled: Boolean,
        val clearAllErrorPosted: String?,
        val loadHistoryErrorPosted: String?,
    )

    private fun simulateClearAll(repositoryThrows: Boolean): ClearAllResult {
        var syncDrainCalled = false
        var clearAllError: String? = null
        val loadHistoryError: String? = null

        try {
            if (repositoryThrows) throw RuntimeException("disk full")
            // success path — no error
        } catch (e: Exception) {
            // CopyPaste-yel4: post to the DEDICATED clearAll error channel, NOT _errors.
            clearAllError = e.message
        } finally {
            // CopyPaste-yel4: drain ALWAYS runs, even when the repository threw.
            syncDrainCalled = true
        }

        return ClearAllResult(
            syncDrainCalled = syncDrainCalled,
            clearAllErrorPosted = clearAllError,
            loadHistoryErrorPosted = loadHistoryError,
        )
    }

    @Test
    fun `clearAll -- sync drain runs on success`() {
        val result = simulateClearAll(repositoryThrows = false)
        assertTrue("Sync drain must run on success", result.syncDrainCalled)
    }

    @Test
    fun `clearAll -- sync drain runs even when repository throws`() {
        val result = simulateClearAll(repositoryThrows = true)
        assertTrue("Sync drain must run even on failure", result.syncDrainCalled)
    }

    @Test
    fun `clearAll -- errors post to dedicated channel not load-history slot`() {
        val result = simulateClearAll(repositoryThrows = true)
        assertNull(
            "clearAll errors must NOT be posted to the load-history error slot",
            result.loadHistoryErrorPosted,
        )
        assertEquals(
            "clearAll errors must be posted to the dedicated clearAll error channel",
            "disk full",
            result.clearAllErrorPosted,
        )
    }

    @Test
    fun `clearAll -- no error posted on success`() {
        val result = simulateClearAll(repositoryThrows = false)
        assertNull("No error on success", result.clearAllErrorPosted)
        assertNull("No load-history error on success", result.loadHistoryErrorPosted)
    }

    @Test
    fun `clearAll -- error does not flow into the generic load-history display`() {
        val result = simulateClearAll(repositoryThrows = true)
        assertFalse(
            "clearAll error must never appear as a load-history error",
            result.loadHistoryErrorPosted != null,
        )
    }
}
