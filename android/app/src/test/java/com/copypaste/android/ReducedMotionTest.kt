package com.copypaste.android

import com.copypaste.android.ui.isReducedMotion
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM tests for [isReducedMotion] — CopyPaste-5917.13 (A11Y-5).
 *
 * Verifies that the pulse animation is gated correctly on the Android
 * ANIMATOR_DURATION_SCALE system setting. Tests run without Compose
 * runtime (isReducedMotion is a plain function).
 */
class ReducedMotionTest {

    /** Scale == 0.0 → animations disabled → reduced-motion is true. */
    @Test
    fun `isReducedMotion true when scale is zero`() {
        assertTrue("scale=0.0f should trigger reduced-motion", isReducedMotion(0f))
    }

    /** Scale == 1.0 (system default) → animations on → reduced-motion is false. */
    @Test
    fun `isReducedMotion false when scale is one`() {
        assertFalse("scale=1.0f should not trigger reduced-motion", isReducedMotion(1f))
    }

    /** Scale == 0.5 (half-speed) → animations on → reduced-motion is false. */
    @Test
    fun `isReducedMotion false when scale is half speed`() {
        assertFalse("scale=0.5f should not trigger reduced-motion", isReducedMotion(0.5f))
    }

    /** Scale == 2.0 (double-speed) → animations on → reduced-motion is false. */
    @Test
    fun `isReducedMotion false when scale is double speed`() {
        assertFalse("scale=2.0f should not trigger reduced-motion", isReducedMotion(2f))
    }
}
