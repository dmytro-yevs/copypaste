package com.copypaste.android

import android.os.Build
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Unit tests for [NotificationPermissionHelper.notificationStatus] and
 * [NotificationPermissionHelper.cameraStatus] — the pure permission-status
 * state machines behind [PermissionStatus] (S10 Wave A). Pure functions with
 * DI-seam parameters (sdkInt / isGranted / wasRequested / shouldShowRationale)
 * so they are testable in the :app JVM test module (no Context/Activity/
 * AndroidKeyStore available there).
 */
class NotificationPermissionHelperStatusTest {

    // ── notificationStatus: SDK gating ──────────────────────────────────────

    @Test
    fun `pre-Tiramisu notification permission is NOT_APPLICABLE regardless of other flags`() {
        assertEquals(
            PermissionStatus.NOT_APPLICABLE,
            NotificationPermissionHelper.notificationStatus(
                sdkInt = Build.VERSION_CODES.S_V2,
                isGranted = false,
                wasRequested = true,
                shouldShowRationale = false,
            ),
        )
    }

    @Test
    fun `Tiramisu and granted is GRANTED`() {
        assertEquals(
            PermissionStatus.GRANTED,
            NotificationPermissionHelper.notificationStatus(
                sdkInt = Build.VERSION_CODES.TIRAMISU,
                isGranted = true,
                wasRequested = false,
                shouldShowRationale = false,
            ),
        )
    }

    @Test
    fun `Tiramisu not granted and never requested is DENIED (first-run state)`() {
        assertEquals(
            PermissionStatus.DENIED,
            NotificationPermissionHelper.notificationStatus(
                sdkInt = Build.VERSION_CODES.TIRAMISU,
                isGranted = false,
                wasRequested = false,
                shouldShowRationale = false,
            ),
        )
    }

    @Test
    fun `Tiramisu not granted, requested once, rationale still shown is DENIED (not yet permanent)`() {
        assertEquals(
            PermissionStatus.DENIED,
            NotificationPermissionHelper.notificationStatus(
                sdkInt = Build.VERSION_CODES.TIRAMISU,
                isGranted = false,
                wasRequested = true,
                shouldShowRationale = true,
            ),
        )
    }

    @Test
    fun `Tiramisu not granted, requested, rationale no longer shown is PERMANENTLY_DENIED`() {
        assertEquals(
            PermissionStatus.PERMANENTLY_DENIED,
            NotificationPermissionHelper.notificationStatus(
                sdkInt = Build.VERSION_CODES.TIRAMISU,
                isGranted = false,
                wasRequested = true,
                shouldShowRationale = false,
            ),
        )
    }

    // ── cameraStatus: same state machine, no SDK gating ─────────────────────

    @Test
    fun `camera granted is GRANTED`() {
        assertEquals(
            PermissionStatus.GRANTED,
            NotificationPermissionHelper.cameraStatus(
                isGranted = true,
                wasRequested = false,
                shouldShowRationale = false,
            ),
        )
    }

    @Test
    fun `camera never requested is DENIED`() {
        assertEquals(
            PermissionStatus.DENIED,
            NotificationPermissionHelper.cameraStatus(
                isGranted = false,
                wasRequested = false,
                shouldShowRationale = false,
            ),
        )
    }

    @Test
    fun `camera requested with rationale still available is DENIED`() {
        assertEquals(
            PermissionStatus.DENIED,
            NotificationPermissionHelper.cameraStatus(
                isGranted = false,
                wasRequested = true,
                shouldShowRationale = true,
            ),
        )
    }

    @Test
    fun `camera requested with rationale suppressed is PERMANENTLY_DENIED`() {
        assertEquals(
            PermissionStatus.PERMANENTLY_DENIED,
            NotificationPermissionHelper.cameraStatus(
                isGranted = false,
                wasRequested = true,
                shouldShowRationale = false,
            ),
        )
    }

    // ── PermissionStatus.isSatisfied() bridge ───────────────────────────────

    @Test
    fun `isSatisfied is true only for GRANTED and NOT_APPLICABLE`() {
        assertEquals(true, PermissionStatus.GRANTED.isSatisfied())
        assertEquals(true, PermissionStatus.NOT_APPLICABLE.isSatisfied())
        assertEquals(false, PermissionStatus.DENIED.isSatisfied())
        assertEquals(false, PermissionStatus.PERMANENTLY_DENIED.isSatisfied())
    }
}
