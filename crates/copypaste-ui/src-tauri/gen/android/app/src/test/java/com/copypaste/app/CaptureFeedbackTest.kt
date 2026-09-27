package com.copypaste.app

import android.app.Notification
import android.app.NotificationManager
import android.media.AudioManager
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [28])
class CaptureFeedbackTest {
    @Test
    @Suppress("DEPRECATION")
    fun notificationFeedbackHonoursRingerMuteAndVolume() {
        var queued = 0
        val play = { queued += 1 }

        assertTrue(CaptureFeedback.playIfAllowed(AudioManager.RINGER_MODE_NORMAL, false, 4, play))
        assertFalse(CaptureFeedback.playIfAllowed(AudioManager.RINGER_MODE_SILENT, false, 4, play))
        assertFalse(CaptureFeedback.playIfAllowed(AudioManager.RINGER_MODE_VIBRATE, false, 4, play))
        assertFalse(CaptureFeedback.playIfAllowed(AudioManager.RINGER_MODE_NORMAL, true, 4, play))
        assertFalse(CaptureFeedback.playIfAllowed(AudioManager.RINGER_MODE_NORMAL, false, 0, play))
        assertEquals(1, queued)
    }

    @Test
    fun requiredOngoingAndRecoveryNotificationsRemain() {
        val context = RuntimeEnvironment.getApplication()
        val ongoing = CaptureNotifications.ongoing(context, "Saving copies")
        assertTrue(ongoing.flags and Notification.FLAG_ONGOING_EVENT != 0)

        CaptureNotifications.postLost(context, "Capture stopped", "Restart Shizuku")
        val manager = context.getSystemService(NotificationManager::class.java)
        assertEquals(1, shadowOf(manager).allNotifications.size)
    }
}
