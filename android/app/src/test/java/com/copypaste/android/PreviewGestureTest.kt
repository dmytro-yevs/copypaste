package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * CopyPaste-vp63.42 — unit test for the pure long-press peek/pin phase
 * transition table extracted (verbatim) into PreviewGesture.kt.
 */
class PreviewGestureTest {

    @Test
    fun `idle stays idle regardless of drag or release`() {
        assertEquals(
            PreviewPhase.Idle,
            nextPreviewPhase(PreviewPhase.Idle, dragUpDp = 0f, released = false),
        )
        assertEquals(
            PreviewPhase.Idle,
            nextPreviewPhase(PreviewPhase.Idle, dragUpDp = 999f, released = true),
        )
    }

    @Test
    fun `peeking stays peeking below the commit threshold without release`() {
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
    fun `peeking commits to pinned once the drag threshold is reached`() {
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
    fun `peeking commits to pinned when the drag exceeds the threshold`() {
        assertEquals(
            PreviewPhase.Pinned,
            nextPreviewPhase(
                PreviewPhase.Peeking,
                dragUpDp = COMMIT_DRAG_THRESHOLD_DP + 10f,
                released = false,
            ),
        )
    }

    @Test
    fun `peeking returns to idle on release without enough upward drag`() {
        assertEquals(
            PreviewPhase.Idle,
            nextPreviewPhase(
                PreviewPhase.Peeking,
                dragUpDp = 5f,
                released = true,
            ),
        )
    }

    @Test
    fun `threshold takes priority over release in the same transition`() {
        // Mirrors the gesture Modifier: dragUpDp is checked before released.
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
    fun `pinned stays pinned regardless of drag or release`() {
        assertEquals(
            PreviewPhase.Pinned,
            nextPreviewPhase(PreviewPhase.Pinned, dragUpDp = 0f, released = false),
        )
        assertEquals(
            PreviewPhase.Pinned,
            nextPreviewPhase(PreviewPhase.Pinned, dragUpDp = 500f, released = true),
        )
    }
}
