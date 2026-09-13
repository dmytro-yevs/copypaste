package com.copypaste.app

import android.app.Service
import android.content.Context
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33])
class CaptureServiceTest {
    @Before
    fun resetPersistedCapturePreference() {
        org.robolectric.RuntimeEnvironment.getApplication()
            .getSharedPreferences("capture-service", Context.MODE_PRIVATE)
            .edit()
            .clear()
            .commit()
    }

    @Test
    fun aFreshInstallWantsCaptureOnAndPersistsThatDefault() {
        val context = org.robolectric.RuntimeEnvironment.getApplication()
        assertTrue(CaptureService.userWantsCapture(context))
        assertTrue(CaptureService.userWantsCapture(context))
    }

    @Test
    fun aStickyRestartWithoutCopyDoesNotWriteThePreferenceOff() {
        val context = org.robolectric.RuntimeEnvironment.getApplication()
        assertTrue(CaptureService.userWantsCapture(context))
        val controller = Robolectric.buildService(CaptureService::class.java)
        val service = controller.create().get()
        val result = service.onStartCommand(null, 0, 7)
        assertEquals(Service.START_NOT_STICKY, result)
        assertTrue(CaptureService.userWantsCapture(context))
        controller.destroy()
        assertTrue(CaptureService.userWantsCapture(context))
    }

    @Test
    fun onlyAnExplicitStopPersistsCaptureOff() {
        val context = org.robolectric.RuntimeEnvironment.getApplication()
        assertTrue(
            CaptureService.rememberArm(
                context,
                CaptureArmRequest("ongoing", "stopped", "body"),
            ),
        )
        assertTrue(CaptureService.userWantsCapture(context))
        assertTrue(CaptureService.isArmed(context))
        CaptureService.stop(context)
        assertFalse(CaptureService.userWantsCapture(context))
        assertFalse(CaptureService.isArmed(context))
    }

    @Test
    fun aFailedStartKeepsTheWantedPreference() {
        val context = org.robolectric.RuntimeEnvironment.getApplication()
        assertFalse(
            CaptureService.start(
                context,
                CaptureArmRequest("ongoing", "stopped", "body"),
            ) && ClipCascadeCapture.isListening(),
        )
        assertTrue(CaptureService.userWantsCapture(context))
    }

    @Test
    fun incompleteRustCopyCannotBecomePersistedServiceState() {
        val context = org.robolectric.RuntimeEnvironment.getApplication()
        assertFalse(CaptureService.start(context, CaptureArmRequest("", "stopped", "body")))
        assertFalse(CaptureService.isArmed(context))
        assertTrue(CaptureService.userWantsCapture(context))
    }
}
