package com.copypaste.app

import android.app.Notification
import android.app.NotificationManager
import android.app.Service
import android.os.Build
import androidx.core.app.NotificationCompat
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [24, 26, 36])
class BackgroundActivityNotificationTest {
    private val manager: NotificationManager
        get() = RuntimeEnvironment.getApplication().getSystemService(NotificationManager::class.java)

    @Test fun allBackgroundServicesShareOneNotificationInEveryStartOrder() {
        // Attach the actual service types without starting their native monitors.
        val services = listOf<Service>(
            Robolectric.buildService(ClipboardCaptureService::class.java).get(),
            Robolectric.buildService(ScreenshotCaptureService::class.java).get(),
            Robolectric.buildService(SmsModuleService::class.java).get(),
        )
        for (first in services) {
            for (second in services.filter { it !== first }) {
                val third = services.single { it !== first && it !== second }
                for (service in listOf(first, second, third, first)) {
                    BackgroundActivityNotification.start(service)
                    assertEquals(1, manager.activeNotifications.size)
                    assertEquals(
                        shadowOf(first).lastForegroundNotificationId,
                        shadowOf(service).lastForegroundNotificationId,
                    )
                }
            }
        }
    }

    @Test fun backgroundStatusIsSilentOngoingAndOpensCopyPaste() {
        val service = Robolectric.buildService(ScreenshotCaptureService::class.java).get()
        BackgroundActivityNotification.start(service)
        val notification = manager.activeNotifications.single().notification
        assertEquals("CopyPaste is active", notification.extras.getString(Notification.EXTRA_TITLE))
        assertTrue(notification.flags and Notification.FLAG_ONGOING_EVENT != 0)
        assertTrue(notification.flags and Notification.FLAG_ONLY_ALERT_ONCE != 0)
        assertNull(notification.sound)
        assertNull(notification.vibrate)
        assertEquals(0, notification.defaults and (Notification.DEFAULT_SOUND or Notification.DEFAULT_VIBRATE))
        assertEquals(
            MainActivity::class.java.name,
            shadowOf(notification.contentIntent).savedIntent.component?.className,
        )
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = manager.getNotificationChannel(notification.channelId)
            assertEquals(NotificationManager.IMPORTANCE_LOW, channel.importance)
            assertNull(channel.sound)
            assertFalse(channel.shouldVibrate())
        }
    }

    @Test fun startingBackgroundWorkDoesNotReplaceOptionalClipboardFeedback() {
        val app = RuntimeEnvironment.getApplication()
        val feedback = NotificationCompat.Builder(app, "clipboard-capture-events-silent")
            .setSmallIcon(R.mipmap.ic_launcher)
            .setContentTitle("Clipboard saved")
            .build()
        manager.notify(2300, feedback)
        BackgroundActivityNotification.start(Robolectric.buildService(SmsModuleService::class.java).get())
        BackgroundActivityNotification.start(Robolectric.buildService(ScreenshotCaptureService::class.java).get())
        assertEquals(2, manager.activeNotifications.size)
        assertEquals(
            "Clipboard saved",
            manager.activeNotifications.single { it.id == 2300 }
                .notification.extras.getString(Notification.EXTRA_TITLE),
        )
    }
}
