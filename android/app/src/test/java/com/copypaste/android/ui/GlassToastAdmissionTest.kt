package com.copypaste.android.ui

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * CopyPaste-myh8.11 S11 Wave 1 — pure decision-table tests for
 * [decideToastAdmission], the coalescing rule that decides whether a new
 * toast REPLACEs the current one, is DROPped, or PROMOTEs an actionable
 * toast to keep it visible when a lower-priority toast would otherwise
 * clobber it. No coroutines/Compose involved — plain JUnit over a pure fn.
 */
class GlassToastAdmissionTest {

    @Test
    fun `no toast present replaces`() {
        assertEquals(
            ToastAdmission.REPLACE,
            decideToastAdmission(currentPresent = false, currentIsActionable = false, newIsActionable = false),
        )
    }

    @Test
    fun `non-actionable current is replaced regardless of new`() {
        assertEquals(
            ToastAdmission.REPLACE,
            decideToastAdmission(currentPresent = true, currentIsActionable = false, newIsActionable = true),
        )
    }

    @Test
    fun `actionable current with non-actionable new is dropped`() {
        assertEquals(
            ToastAdmission.DROP,
            decideToastAdmission(currentPresent = true, currentIsActionable = true, newIsActionable = false),
        )
    }

    @Test
    fun `actionable current with actionable new is promoted`() {
        assertEquals(
            ToastAdmission.PROMOTE,
            decideToastAdmission(currentPresent = true, currentIsActionable = true, newIsActionable = true),
        )
    }
}
