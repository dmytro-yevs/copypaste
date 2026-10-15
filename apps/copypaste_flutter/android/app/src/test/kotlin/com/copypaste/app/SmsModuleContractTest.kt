package com.copypaste.app

import android.content.ComponentName
import android.content.pm.PackageManager
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class SmsModuleContractTest {
    @Test
    fun unavailableHandlerDoesNotDisableMonitoringOrClearItsCursor() {
        val context = RuntimeEnvironment.getApplication()
        val preferences = context.getSharedPreferences("sms-module-inbox", 0)
        preferences.edit().putLong("activated-at", 100).putLong("last-id", 42).commit()
        val receiver = ComponentName(context, SmsModuleReceiver::class.java)
        context.packageManager.setComponentEnabledSetting(
            receiver,
            PackageManager.COMPONENT_ENABLED_STATE_ENABLED,
            PackageManager.DONT_KILL_APP,
        )

        assertFalse(SmsModuleService.synchronize(context, NativeSmsModules.HANDLER_UNAVAILABLE))

        assertEquals(100, preferences.getLong("activated-at", 0))
        assertEquals(42, preferences.getLong("last-id", 0))
        assertEquals(
            PackageManager.COMPONENT_ENABLED_STATE_ENABLED,
            context.packageManager.getComponentEnabledSetting(receiver),
        )
    }

    @Test
    fun acknowledgementRunsAfterIngestAndPreservesRetryableSmsRows() {
        val context = RuntimeEnvironment.getApplication()
        val preferences = context.getSharedPreferences("sms-module-acknowledgement", 0)
        preferences.edit().putLong("last-id", 41).commit()

        var observedLastId = 0L
        assertFalse(SmsInboxMonitor.ingestAndAcknowledge(preferences, 42) {
            observedLastId = preferences.getLong("last-id", 0)
            NativeSmsModules.INGEST_RETRY
        })
        assertEquals(41, observedLastId)
        assertEquals(41, preferences.getLong("last-id", 0))

        assertFalse(SmsInboxMonitor.ingestAndAcknowledge(preferences, 42) {
            throw IllegalStateException("transient native failure")
        })
        assertEquals(41, preferences.getLong("last-id", 0))

        assertTrue(SmsInboxMonitor.ingestAndAcknowledge(preferences, 42) {
            NativeSmsModules.INGEST_CAPTURED
        })
        assertEquals(42, preferences.getLong("last-id", 0))

        assertTrue(SmsInboxMonitor.ingestAndAcknowledge(preferences, 43) {
            NativeSmsModules.INGEST_IGNORED
        })
        assertEquals(43, preferences.getLong("last-id", 0))
    }
}
