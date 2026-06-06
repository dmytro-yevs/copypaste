package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Pure JVM unit tests for [nextPreviewPhase] — the preview gesture state-machine.
 *
 * No Android SDK or Compose runtime required; runs via JVM unit test runner.
 */
class PreviewPhaseTest {

    // ── Idle ─────────────────────────────────────────────────────────────────

    @Test
    fun idle_staysIdle_onAnyDrag() {
        assertEquals(
            PreviewPhase.Idle,
            nextPreviewPhase(PreviewPhase.Idle, dragUpDp = 100f, released = false),
        )
    }

    @Test
    fun idle_staysIdle_onRelease() {
        assertEquals(
            PreviewPhase.Idle,
            nextPreviewPhase(PreviewPhase.Idle, dragUpDp = 0f, released = true),
        )
    }

    // ── Peeking ───────────────────────────────────────────────────────────────

    @Test
    fun peeking_staysPeeking_belowThreshold_notReleased() {
        assertEquals(
            PreviewPhase.Peeking,
            nextPreviewPhase(
                PreviewPhase.Peeking,
                dragUpDp = COMMIT_DRAG_THRESHOLD_DP - 1f,
                released = false,
            ),
        )
    }

    @Test
    fun peeking_transitionsToPinned_atExactThreshold() {
        assertEquals(
            PreviewPhase.Pinned,
            nextPreviewPhase(
                PreviewPhase.Peeking,
                dragUpDp = COMMIT_DRAG_THRESHOLD_DP,
                released = false,
            ),
        )
    }

    @Test
    fun peeking_transitionsToPinned_aboveThreshold() {
        assertEquals(
            PreviewPhase.Pinned,
            nextPreviewPhase(
                PreviewPhase.Peeking,
                dragUpDp = COMMIT_DRAG_THRESHOLD_DP + 50f,
                released = false,
            ),
        )
    }

    @Test
    fun peeking_transitionsToIdle_onRelease_belowThreshold() {
        assertEquals(
            PreviewPhase.Idle,
            nextPreviewPhase(
                PreviewPhase.Peeking,
                dragUpDp = COMMIT_DRAG_THRESHOLD_DP - 1f,
                released = true,
            ),
        )
    }

    @Test
    fun peeking_transitionsToPinned_onRelease_atThreshold() {
        // Commit takes priority over release when drag already crossed threshold.
        assertEquals(
            PreviewPhase.Pinned,
            nextPreviewPhase(
                PreviewPhase.Peeking,
                dragUpDp = COMMIT_DRAG_THRESHOLD_DP,
                released = true,
            ),
        )
    }

    @Test
    fun peeking_transitionsToIdle_onReleaseWithNoDrag() {
        assertEquals(
            PreviewPhase.Idle,
            nextPreviewPhase(PreviewPhase.Peeking, dragUpDp = 0f, released = true),
        )
    }

    // ── Pinned ────────────────────────────────────────────────────────────────

    @Test
    fun pinned_staysPinned_onDrag() {
        assertEquals(
            PreviewPhase.Pinned,
            nextPreviewPhase(PreviewPhase.Pinned, dragUpDp = 200f, released = false),
        )
    }

    @Test
    fun pinned_staysPinned_onRelease() {
        // Pinned only dismisses via explicit onDismiss callback, not gesture release.
        assertEquals(
            PreviewPhase.Pinned,
            nextPreviewPhase(PreviewPhase.Pinned, dragUpDp = 0f, released = true),
        )
    }
}
