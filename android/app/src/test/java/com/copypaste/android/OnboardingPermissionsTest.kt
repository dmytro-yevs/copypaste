package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-vp63.41 — [RequestInFlightGate] is the pure state machine backing
 * [OnboardingPermissions.launchGated] / requestNotificationPermission: at most
 * one permission/settings request may be in flight at a time, and further
 * taps are ignored until the current one completes.
 */
class OnboardingPermissionsTest {

    @Test
    fun `gate starts free`() {
        val gate = RequestInFlightGate()
        assertFalse(gate.isInFlight)
    }

    @Test
    fun `acquire marks the gate in-flight`() {
        val gate = RequestInFlightGate()
        gate.acquire()
        assertTrue(gate.isInFlight)
    }

    @Test
    fun `release frees a previously acquired gate`() {
        val gate = RequestInFlightGate()
        gate.acquire()
        gate.release()
        assertFalse(gate.isInFlight)
    }

    @Test
    fun `release is idempotent on an already-free gate`() {
        val gate = RequestInFlightGate()
        gate.release()
        assertFalse(gate.isInFlight)
    }

    @Test
    fun `acquire is idempotent while already in-flight`() {
        val gate = RequestInFlightGate()
        gate.acquire()
        gate.acquire()
        assertTrue(gate.isInFlight)
    }
}
