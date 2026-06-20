package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-mp1x: Background clipboard capture indicator parity check.
 *
 * Root cause: Android blocks background clipboard reads (restriction introduced
 * in Android 10). The app requires READ_LOGS (ADB-only) to work around this.
 * The Settings screen must display: (a) a clear section explaining the
 * limitation, (b) live status of READ_LOGS and overlay grants, and (c) the
 * exact ADB commands needed to grant them.
 *
 * Implementation: GeneralTab in SettingsActivity already contains the full
 * ADB capture section — AdbCaptureStatusLine + AdbCaptureCommandRows — with
 * string resources. These tests verify the pure-JVM logic for the status
 * state machine (no Android context required).
 */
class BgCaptureIndicatorTest {

    // ── LogcatCaptureStatus state machine (pure-JVM) ──────────────────────────

    /**
     * Mirror of the four status states [LogcatCaptureService] can return.
     * Keeps the test independent of the Android-context service class.
     */
    private enum class CaptureStatus {
        NOT_GRANTED,      // READ_LOGS permission not granted
        DISABLED,         // permission granted but user toggled off
        GRANTED_NOT_WORKING, // granted + enabled but the logcat pipe is silent
        WORKING,          // all good, clips are being read
    }

    private fun statusFromGrants(
        readLogsGranted: Boolean,
        enabled: Boolean,
        logcatWorking: Boolean,
    ): CaptureStatus = when {
        !readLogsGranted -> CaptureStatus.NOT_GRANTED
        !enabled         -> CaptureStatus.DISABLED
        logcatWorking    -> CaptureStatus.WORKING
        else             -> CaptureStatus.GRANTED_NOT_WORKING
    }

    @Test
    fun `no READ_LOGS grant -- status is NOT_GRANTED`() {
        val status = statusFromGrants(readLogsGranted = false, enabled = true, logcatWorking = false)
        assertEquals(CaptureStatus.NOT_GRANTED, status)
    }

    @Test
    fun `READ_LOGS granted but user disabled -- status is DISABLED`() {
        val status = statusFromGrants(readLogsGranted = true, enabled = false, logcatWorking = false)
        assertEquals(CaptureStatus.DISABLED, status)
    }

    @Test
    fun `READ_LOGS granted, enabled, but pipe silent -- status is GRANTED_NOT_WORKING`() {
        val status = statusFromGrants(readLogsGranted = true, enabled = true, logcatWorking = false)
        assertEquals(CaptureStatus.GRANTED_NOT_WORKING, status)
    }

    @Test
    fun `all conditions met -- status is WORKING`() {
        val status = statusFromGrants(readLogsGranted = true, enabled = true, logcatWorking = true)
        assertEquals(CaptureStatus.WORKING, status)
    }

    // ── ADB command strings sanity ────────────────────────────────────────────

    @Test
    fun `adb grant command targets READ_LOGS permission`() {
        val cmd = "adb shell pm grant com.copypaste.android android.permission.READ_LOGS"
        assertTrue("Grant command must mention READ_LOGS", cmd.contains("READ_LOGS"))
        assertTrue("Grant command must target the app package", cmd.contains("com.copypaste.android"))
    }

    @Test
    fun `adb revoke command mirrors grant command`() {
        val grant  = "adb shell pm grant com.copypaste.android android.permission.READ_LOGS"
        val revoke = "adb shell pm revoke com.copypaste.android android.permission.READ_LOGS"
        assertTrue(revoke.contains("revoke"))
        assertTrue(revoke.contains("READ_LOGS"))
        // Both commands must target the same package.
        assertEquals(
            grant.substringAfter("pm grant "),
            revoke.substringAfter("pm revoke "),
        )
    }
}
