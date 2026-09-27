package com.copypaste.app

import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

class ClipCascadeCaptureTest {
    @Test
    fun activeReaderIsReusedAndNeverStartsASuccessor() {
        val gate = ClipCascadeRunGate<Any>()
        val first = Any()
        assertSame(first, gate.begin(first))
        assertNull(gate.begin(Any()))
        assertTrue(gate.owns(first))
    }

    @Test
    fun stopBeforeReaderStartupFencesItsLaterCallbacks() {
        val gate = ClipCascadeRunGate<Any>()
        val old = Any()
        gate.begin(old)
        assertSame(old, gate.stop())
        val replacement = Any()
        gate.begin(replacement)

        assertFalse(gate.owns(old))
        assertFalse(gate.finish(old))
        assertSame(replacement, gate.active())
    }

    @Test
    fun unexpectedExitOnlyClaimsTheCurrentRun() {
        val gate = ClipCascadeRunGate<Any>()
        val run = Any()
        gate.begin(run)
        assertTrue(gate.finish(run))
        assertNull(gate.active())
    }
}
