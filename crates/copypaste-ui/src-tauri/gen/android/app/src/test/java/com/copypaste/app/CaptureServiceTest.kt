package com.copypaste.app

import android.content.Intent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33])
class CaptureServiceTest {
    @Test
    fun aStickyRestartWithoutAnIntentStopsAndDoesNotStayForeground() {
        val controller = Robolectric.buildService(CaptureService::class.java)
        val service = controller.create().get()
        val result = service.onStartCommand(null, 0, 7)
        assertEquals(android.app.Service.START_NOT_STICKY, result)
        controller.destroy()
    }

    @Test
    fun isArmedIsFalseUntilStartPersistsState() {
        val context = org.robolectric.RuntimeEnvironment.getApplication()
        assertFalse(CaptureService.isArmed(context))
        assertFalse(
            CaptureService.start(
                context,
                CaptureArmRequest("ongoing", "stopped", "body"),
            ),
        )
        assertFalse(CaptureService.isArmed(context))
    }

    @Test
    fun incompleteRustCopyCannotBecomePersistedServiceState() {
        val context = org.robolectric.RuntimeEnvironment.getApplication()
        assertFalse(CaptureService.start(context, CaptureArmRequest("", "stopped", "body")))
        assertFalse(CaptureService.isArmed(context))
    }
}
