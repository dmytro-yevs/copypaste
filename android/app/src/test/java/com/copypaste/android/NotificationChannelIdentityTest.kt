package com.copypaste.android

import android.app.NotificationManager
import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows
import org.robolectric.annotation.Config

/**
 * S12 W1: characterization tests for notification channel identity (id +
 * importance) and the PendingIntent targets wired into each notification
 * surface. Channel importance/visibility CANNOT change post-creation on a
 * real device — these tests pin the current (correct) values so a future
 * edit that accidentally changes importance fails loudly.
 *
 * This module has `isIncludeAndroidResources` unset for JVM unit tests (see
 * [RepairedSettingsConsumersTest] / [NotificationsTabSettingsTest] kdocs for
 * the same, pre-existing constraint) — Robolectric has no merged resource
 * table, so any real `context.getString(R.string...)` call throws
 * `Resources$NotFoundException`. [ensureChannel]/[buildNotification]/
 * [postIncomingPairNotification]/[NotificationHelper.createChannels] all call
 * `getString` for channel/notification text. Rather than skip exercising the
 * real channel-creation/PendingIntent-building logic, [StringStubContext]
 * wraps the real Robolectric context and stubs ONLY `getString`, delegating
 * everything else (NotificationManager, SharedPreferences, etc.) to the real
 * context — so the production channel-id/importance/PendingIntent logic under
 * test is exercised unmodified; only the resource-text lookup is faked.
 * [StringStubContext] is a shared test helper (also used by
 * [RepairedSettingsConsumersTest] for the same reason).
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class NotificationChannelIdentityTest {

    private fun stubContext(): Context = StringStubContext(ApplicationProvider.getApplicationContext())

    private fun notificationManager(context: Context): NotificationManager =
        context.getSystemService(NotificationManager::class.java)

    @Test
    fun `ServiceNotifications ensureChannel creates the three expected channels with pinned importance`() {
        val context = stubContext()
        ServiceNotifications.ensureChannel(context)

        val nm = notificationManager(context)
        val service = nm.getNotificationChannel("copypaste_service")
        val copyEvent = nm.getNotificationChannel("copypaste_copy_event")
        val pairRequest = nm.getNotificationChannel("copypaste_pair_request")

        assertNotNull(service)
        assertNotNull(copyEvent)
        assertNotNull(pairRequest)
        assertEquals(NotificationManager.IMPORTANCE_LOW, service!!.importance)
        assertEquals(NotificationManager.IMPORTANCE_MIN, copyEvent!!.importance)
        assertEquals(NotificationManager.IMPORTANCE_HIGH, pairRequest!!.importance)
    }

    @Test
    fun `ServiceNotifications ensureChannel is idempotent`() {
        val context = stubContext()
        ServiceNotifications.ensureChannel(context)
        ServiceNotifications.ensureChannel(context)

        val nm = notificationManager(context)
        val countBefore = nm.notificationChannels.size
        ServiceNotifications.ensureChannel(context)
        assertEquals(countBefore, nm.notificationChannels.size)
    }

    @Test
    fun `NotificationHelper createChannels creates copypaste_sync with IMPORTANCE_LOW`() {
        val context = stubContext()
        NotificationHelper.createChannels(context)

        val nm = notificationManager(context)
        val sync = nm.getNotificationChannel("copypaste_sync")
        assertNotNull(sync)
        assertEquals(NotificationManager.IMPORTANCE_LOW, sync!!.importance)
    }

    @Test
    fun `NotificationHelper createChannels is idempotent`() {
        val context = stubContext()
        NotificationHelper.createChannels(context)
        val nm = notificationManager(context)
        val countBefore = nm.notificationChannels.size
        NotificationHelper.createChannels(context)
        assertEquals(countBefore, nm.notificationChannels.size)
    }

    @Test
    fun `buildNotification Open action targets MainActivity`() {
        val context = stubContext()
        val notification = ServiceNotifications.buildNotification(context)

        // Actions are [toggle, open] — the "Open" action is the second one, and
        // its PendingIntent's shadow exposes the wrapped Intent's target class.
        val openAction = notification.actions[1]
        val shadowPi = Shadows.shadowOf(openAction.actionIntent)
        val savedIntent = shadowPi.savedIntent
        assertEquals(MainActivity::class.java.name, savedIntent.component?.className)
    }

    @Test
    fun `buildNotification Pause action broadcasts ACTION_PAUSE when capture is enabled`() {
        val context = stubContext()
        Settings(context).captureEnabled = true

        val notification = ServiceNotifications.buildNotification(context)
        val toggleAction = notification.actions[0]
        val shadowPi = Shadows.shadowOf(toggleAction.actionIntent)
        val savedIntent = shadowPi.savedIntent

        assertEquals(CaptureControlReceiver.ACTION_PAUSE, savedIntent.action)
    }

    @Test
    fun `buildNotification Resume action broadcasts ACTION_RESUME when capture is paused`() {
        val context = stubContext()
        Settings(context).captureEnabled = false

        val notification = ServiceNotifications.buildNotification(context)
        val toggleAction = notification.actions[0]
        val shadowPi = Shadows.shadowOf(toggleAction.actionIntent)
        val savedIntent = shadowPi.savedIntent

        assertEquals(CaptureControlReceiver.ACTION_RESUME, savedIntent.action)
    }

    @Test
    fun `postIncomingPairNotification contentIntent targets DevicesActivity with EXTRA_AUTO_OPEN_SAS true`() {
        val context = stubContext()
        ServiceNotifications.postIncomingPairNotification(context, "Some Peer")

        val nm = notificationManager(context)
        val posted = nm.activeNotifications.first { it.id == ServiceNotifications.NOTIF_ID_PAIR_REQUEST }
        val contentIntentShadow = Shadows.shadowOf(posted.notification.contentIntent)
        val savedIntent = contentIntentShadow.savedIntent

        assertEquals(DevicesActivity::class.java.name, savedIntent.component?.className)
        assertEquals(true, savedIntent.getBooleanExtra(DevicesActivity.EXTRA_AUTO_OPEN_SAS, false))
    }
}
