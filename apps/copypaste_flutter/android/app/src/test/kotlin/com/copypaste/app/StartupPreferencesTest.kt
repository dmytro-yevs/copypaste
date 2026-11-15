package com.copypaste.app

import android.app.Application
import android.content.Intent
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class StartupPreferencesTest {
    private lateinit var app: Application

    @Before fun setup() {
        app = RuntimeEnvironment.getApplication()
        app.getSharedPreferences("copypaste_startup", 0).edit().clear().commit()
    }

    @Test fun preservesExistingBootBehaviorAndPersistsUserChoice() {
        assertTrue(StartupPreferences.enabled(app))
        assertTrue(StartupPreferences.setEnabled(app, false))
        assertFalse(StartupPreferences.enabled(app))
        assertTrue(StartupPreferences.setEnabled(app, true))
        assertTrue(StartupPreferences.enabled(app))
    }

    @Test fun disabledStartupDoesNotLaunchServicesOrRuntimeAtBoot() {
        StartupPreferences.setEnabled(app, false)
        val boot = Intent(Intent.ACTION_BOOT_COMPLETED)
        ScreenshotCaptureBootReceiver().onReceive(app, boot)
        SmsModuleReceiver().onReceive(app, boot)
        assertNull(shadowOf(app).nextStartedService)
    }
}
