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
        CaptureService.start(context, "Capturing from every app.", "stopped", "body")
        val started = Robolectric.buildService(
            CaptureService::class.java,
            Intent(context, CaptureService::class.java).putExtra("text", "Capturing from every app."),
        ).create().get()
        assertEquals(
            android.app.Service.START_NOT_STICKY,
            started.onStartCommand(null, 0, 1),
        )
    }
}
