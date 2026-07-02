package com.copypaste.android.ui.theme

import androidx.compose.animation.core.SnapSpec
import androidx.compose.animation.core.spring
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * android-design-system "CpMotion durations and system-driven reduced motion"
 * requirement: fixed 120/200/300ms durations, and reduced motion MUST disable
 * a spring entirely (not merely zero its duration).
 */
class CpMotionTest {

    @Test
    fun `durations are fixed at 120 200 300ms`() {
        assertEquals(120, CpMotion.FAST_MS)
        assertEquals(200, CpMotion.DEFAULT_MS)
        assertEquals(300, CpMotion.THEME_MS)
    }

    @Test
    fun `cpMotionDuration collapses to 0 only when reduced`() {
        assertEquals(120, cpMotionDuration(CpMotion.FAST_MS, reduced = false))
        assertEquals(0, cpMotionDuration(CpMotion.FAST_MS, reduced = true))
        assertEquals(0, cpMotionDuration(CpMotion.THEME_MS, reduced = true))
    }

    @Test
    fun `cpMotionSpec swaps the spring for a hard snap under reduced motion`() {
        val reducedSpec = cpMotionSpec(reduced = true) { spring<Float>() }
        val normalSpec = cpMotionSpec(reduced = false) { spring<Float>() }
        assertTrue("reduced motion must select a SnapSpec, not a de-tuned spring", reducedSpec is SnapSpec<Float>)
        assertTrue("normal motion must keep the spring, not snap", normalSpec !is SnapSpec<Float>)
    }
}
