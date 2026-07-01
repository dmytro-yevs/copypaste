package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-vp63.39 — characterization tests for [DevicesController]'s pure
 * decision-logic helpers, extracted from inline guards in the former
 * `DevicesScreen` god-composable (DevicesActivity.kt). [DevicesController]
 * itself requires an Android [Settings]/[DeviceKeyStore] (Context-backed) and
 * UniFFI native calls, so it is not directly instantiable here — these tests
 * cover only the free-standing, Compose-free predicate functions it calls.
 */
class DevicesControllerTest {

    // ── canStartPairing ───────────────────────────────────────────────────

    @Test
    fun `canStartPairing is true when idle`() {
        assertTrue(canStartPairing(pairStarting = false, pairingPeer = null))
    }

    @Test
    fun `canStartPairing is false while a pairing request is already in flight`() {
        assertFalse(canStartPairing(pairStarting = true, pairingPeer = null))
    }

    @Test
    fun `canStartPairing is false while the SAS modal is already open for another peer`() {
        val peer = DiscoveredPeer(
            deviceId = "d1",
            deviceName = "Mac",
            ipAddrs = listOf("10.0.0.5"),
            port = 1234u,
            bport = 5678u,
            paired = false,
        )
        assertFalse(canStartPairing(pairStarting = false, pairingPeer = peer))
    }

    // ── canDismissRevokeRotate ────────────────────────────────────────────

    @Test
    fun `canDismissRevokeRotate is true when no rotation is in flight`() {
        assertTrue(canDismissRevokeRotate(revokeRotateInFlight = false))
    }

    @Test
    fun `canDismissRevokeRotate is false while a rotation is in flight`() {
        assertFalse(canDismissRevokeRotate(revokeRotateInFlight = true))
    }

    // ── canDismissRevokeAllConfirm ────────────────────────────────────────

    @Test
    fun `canDismissRevokeAllConfirm is true when no bulk revoke is in flight`() {
        assertTrue(canDismissRevokeAllConfirm(revokeAllInFlight = false))
    }

    @Test
    fun `canDismissRevokeAllConfirm is false while a bulk revoke is in flight`() {
        assertFalse(canDismissRevokeAllConfirm(revokeAllInFlight = true))
    }
}
