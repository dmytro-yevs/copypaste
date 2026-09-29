package com.copypaste.app

import android.os.PowerManager
import android.provider.Settings
import org.robolectric.RuntimeEnvironment
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [28, 33])
class BackgroundActivityTest {
    @Test
    fun requestsTheCurrentPackageAndReadsOnlyTheSystemGrant() {
        val context = RuntimeEnvironment.getApplication()
        val power = context.getSystemService(PowerManager::class.java)
        shadowOf(power).setIgnoringBatteryOptimizations(context.packageName, false)

        assertFalse(BackgroundActivity.isAllowed(context))
        val request = BackgroundActivity.settingsIntent(context)
        assertEquals(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, request.action)
        assertEquals("package:${context.packageName}", request.data.toString())
        // Merely constructing or opening the intent is not evidence of approval.
        assertFalse(BackgroundActivity.isAllowed(context))

        shadowOf(power).setIgnoringBatteryOptimizations(context.packageName, true)
        assertTrue(BackgroundActivity.isAllowed(context))
        assertEquals(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS, BackgroundActivity.settingsIntent(context).action)
    }
}
