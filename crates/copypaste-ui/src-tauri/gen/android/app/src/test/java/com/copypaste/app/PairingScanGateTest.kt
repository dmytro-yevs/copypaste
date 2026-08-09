package com.copypaste.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PairingScanGateTest {
    @Test
    fun permissionDenialReleasesTheGateAndAGrantCanRecover() {
        val gate = PairingScanGate()

        assertEquals(ScanStep.REQUEST_PERMISSION, gate.begin(permissionGranted = false))
        assertTrue(gate.inFlight)
        assertEquals(ScanStep.PERMISSION_DENIED, gate.permissionResult(granted = false))
        assertFalse(gate.inFlight)

        assertEquals(ScanStep.START_SCANNER, gate.begin(permissionGranted = true))
        assertTrue(gate.inFlight)
        gate.finish()
        assertFalse(gate.inFlight)
    }

    @Test
    fun concurrentScansAreRefusedUntilCancellationCompletes() {
        val gate = PairingScanGate()

        assertEquals(ScanStep.START_SCANNER, gate.begin(permissionGranted = true))
        assertEquals(ScanStep.BUSY, gate.begin(permissionGranted = true))
        gate.finish()
        assertEquals(ScanStep.START_SCANNER, gate.begin(permissionGranted = true))
    }
}
