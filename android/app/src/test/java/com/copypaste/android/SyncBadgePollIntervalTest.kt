package com.copypaste.android

import com.copypaste.android.ui.POLL_INTERVAL_MS
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-1jms.16: regression guard — Android sync-badge poll interval must match macOS.
 *
 * macOS SyncStatusChip.tsx uses SYNC_POLL_INTERVAL_MS = 2_000 ms (flat, no idle back-off).
 * Android SyncStatusBadge previously used 10_000 ms, causing up to 10 s of badge lag vs
 * macOS's ≤ 2 s (PARITY-SPEC §9). This test pins the value so it cannot silently regress.
 *
 * Pure-JVM — no Android SDK, no Compose runtime.
 */
class SyncBadgePollIntervalTest {

    /**
     * POLL_INTERVAL_MS must equal 2 000 ms to match the macOS SyncStatusChip cadence
     * (SYNC_POLL_INTERVAL_MS = 2_000 in crates/copypaste-ui/src/components/SyncStatusChip.tsx).
     *
     * CopyPaste-1jms.16: the old value was 10_000 ms — if this test breaks, the interval
     * has been changed and PARITY-SPEC §9 must be re-verified before merging.
     */
    @Test
    fun `POLL_INTERVAL_MS equals 2000 ms — matches macOS SyncStatusChip cadence`() {
        assertEquals(
            "Android badge poll interval must match macOS SYNC_POLL_INTERVAL_MS (2 000 ms). " +
                "See CopyPaste-1jms.16 and crates/copypaste-ui/src/components/SyncStatusChip.tsx.",
            2_000L,
            POLL_INTERVAL_MS,
        )
    }

    /**
     * Belt-and-suspenders: the interval must be ≤ 3 000 ms so the badge reflects
     * offline state within the ≤ 3 s acceptance criterion (CopyPaste-1jms.16 §AC).
     * Allows a small tolerance above 2 000 ms if the cadence is ever tuned slightly,
     * without requiring an exact match.
     */
    @Test
    fun `POLL_INTERVAL_MS is at most 3000 ms — satisfies offline-latency acceptance criterion`() {
        assertTrue(
            "POLL_INTERVAL_MS ($POLL_INTERVAL_MS ms) exceeds 3 000 ms; " +
                "offline badge transition would exceed the ≤ 3 s acceptance criterion (CopyPaste-1jms.16).",
            POLL_INTERVAL_MS <= 3_000L,
        )
    }
}
