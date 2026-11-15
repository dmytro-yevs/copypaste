package com.copypaste.app

import android.app.Application
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class BackgroundNotificationContentTest {
    private val app: Application get() = RuntimeEnvironment.getApplication()

    @Test fun bodyReportsRunningFeaturesAndPauseActionFollowsState() {
        val running = backgroundNotificationContent(
            app,
            BackgroundNotificationState(paused = false, captureRunning = true, syncRunning = true),
            now = 0L,
        )
        assertEquals("CopyPaste is active", running.title)
        assertEquals("Capturing copies · Syncing in background", running.text)
        assertEquals("Pause", running.pauseAction)

        val paused = backgroundNotificationContent(
            app,
            BackgroundNotificationState(paused = true, captureRunning = true, syncRunning = false),
            now = 0L,
        )
        assertEquals("Capture paused", paused.title)
        assertEquals("Enabled features are running in the background.", paused.text)
        assertEquals("Resume", paused.pauseAction)

        val unknown = backgroundNotificationContent(
            app,
            BackgroundNotificationState(paused = null, captureRunning = false, syncRunning = true),
            now = 0L,
        )
        assertNull(unknown.pauseAction)
    }

    @Test fun lastSyncIsTheNewestPeerSuccess() {
        val status = JSONObject(
            """{"sync_status":{"peers":[{"last_success_ms":10},{"last_success_ms":30},{}]}}""",
        )
        assertEquals(30L, lastSyncAt(status))
        assertEquals(0L, lastSyncAt(JSONObject()))
        assertEquals(0L, lastSyncAt(null))
    }
}
