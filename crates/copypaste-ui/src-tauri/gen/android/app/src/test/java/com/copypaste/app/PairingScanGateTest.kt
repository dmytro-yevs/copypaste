package com.copypaste.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PairingScanGateTest {
    @Test
    fun cancellationReleasesTheGateAndAScanCanRecover() {
        val gate = PairingScanGate()

        assertEquals(ScanStep.START_SCANNER, gate.begin())
        assertTrue(gate.inFlight)
        gate.finish()
        assertFalse(gate.inFlight)

        assertEquals(ScanStep.START_SCANNER, gate.begin())
        assertTrue(gate.inFlight)
        gate.finish()
        assertFalse(gate.inFlight)
    }

    @Test
    fun concurrentScansAreRefusedUntilCancellationCompletes() {
        val gate = PairingScanGate()

        assertEquals(ScanStep.START_SCANNER, gate.begin())
        assertEquals(ScanStep.BUSY, gate.begin())
        gate.finish()
        assertEquals(ScanStep.START_SCANNER, gate.begin())
    }
}
