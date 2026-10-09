package com.copypaste.app

import android.app.Application
import android.app.AppOpsManager
import android.app.usage.UsageEvents
import android.app.usage.UsageStatsManager
import android.content.pm.ApplicationInfo
import android.content.pm.PackageInfo
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.annotation.Implementation
import org.robolectric.annotation.Implements
import org.robolectric.shadows.ShadowUsageStatsManager.EventBuilder

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class ScreenshotSourceAppTest {
    private lateinit var app: Application
    private val takenAt = System.currentTimeMillis() - 10_000

    @Before fun setup() {
        app = RuntimeEnvironment.getApplication()
        access(AppOpsManager.MODE_ALLOWED)
    }

    @Test fun delayedImportUsesScreenshotTimeInsteadOfTheCurrentlyOpenApp() {
        install("com.example.editor", "Editor")
        event(UsageEvents.Event.ACTIVITY_RESUMED, "com.example.editor", takenAt - 1000)
        event(UsageEvents.Event.ACTIVITY_PAUSED, "com.example.editor", takenAt + 100)
        event(UsageEvents.Event.ACTIVITY_RESUMED, "com.example.other", takenAt + 200)
        val source = ScreenshotSourceApps.resolve(app, takenAt)!!
        assertEquals("com.example.editor", source.packageName)
        assertEquals("Editor", source.name)
        assertNotNull(source.icon)
    }

    @Test fun permissionDeniedOrRevokedLeavesAttributionOptional() {
        event(UsageEvents.Event.ACTIVITY_RESUMED, "com.example.editor", takenAt - 1000)
        assertNotNull(ScreenshotSourceApps.resolve(app, takenAt))
        access(AppOpsManager.MODE_IGNORED)
        assertFalse(ScreenshotSourceApps.accessGranted(app))
        assertNull(ScreenshotSourceApps.resolve(app, takenAt))
        assertTrue(ScreenshotCaptureState.asMap(app)["enabled"]!!)
    }

    @Test fun invisiblePackageKeepsTheKnownPackageWithoutInventingNameOrIcon() {
        event(UsageEvents.Event.ACTIVITY_RESUMED, "com.example.uninstalled", takenAt - 1000)
        val source = ScreenshotSourceApps.resolve(app, takenAt)!!
        assertEquals("com.example.uninstalled", source.packageName)
        assertNull(source.name)
        assertNull(source.icon)
    }

    @Test fun missingOrFutureScreenshotTimeAndEmptyEventsReturnNoSource() {
        assertNull(ScreenshotSourceApps.resolve(app, 0))
        assertNull(ScreenshotSourceApps.resolve(app, Long.MAX_VALUE))
        assertNull(ScreenshotSourceApps.resolve(app, takenAt))
    }

    @Test @Config(shadows = [FailingUsageStats::class]) fun usageProviderFailureReturnsNoSource() {
        assertNull(ScreenshotSourceApps.resolve(app, takenAt))
    }

    @Test fun backgroundedAppsAndSystemScreenshotUiAreNotAttributed() {
        val foreground = ScreenshotForegroundApps()
        foreground.accept(UsageEvents.Event.ACTIVITY_RESUMED, "com.example.editor", "Editor")
        foreground.accept(UsageEvents.Event.ACTIVITY_PAUSED, "com.example.editor", "Editor")
        assertNull(foreground.packageName())
        foreground.accept(UsageEvents.Event.ACTIVITY_RESUMED, "com.android.systemui", "Screenshot")
        assertNull(foreground.packageName())
    }

    @Test fun multipleActivitiesOfOneAppAreAcceptedButSplitScreenIsAmbiguous() {
        val foreground = ScreenshotForegroundApps()
        foreground.accept(UsageEvents.Event.ACTIVITY_RESUMED, "com.example.editor", "First")
        foreground.accept(UsageEvents.Event.ACTIVITY_RESUMED, "com.example.editor", "Second")
        foreground.accept(UsageEvents.Event.ACTIVITY_PAUSED, "com.example.editor", "First")
        assertEquals("com.example.editor", foreground.packageName())
        foreground.accept(UsageEvents.Event.ACTIVITY_RESUMED, "com.example.other", "Other")
        assertNull(foreground.packageName())
        foreground.accept(UsageEvents.Event.ACTIVITY_STOPPED, "com.example.other", "Other")
        assertEquals("com.example.editor", foreground.packageName())
    }

    @Test fun lockScreenAndShutdownClearStaleApps() {
        for (type in listOf(UsageEvents.Event.KEYGUARD_SHOWN, UsageEvents.Event.SCREEN_NON_INTERACTIVE, UsageEvents.Event.DEVICE_SHUTDOWN)) {
            val foreground = ScreenshotForegroundApps()
            foreground.accept(UsageEvents.Event.ACTIVITY_RESUMED, "com.example.editor", "Editor")
            foreground.accept(type, null, null)
            assertNull(foreground.packageName())
        }
    }

    private fun access(mode: Int) {
        shadowOf(app.getSystemService(AppOpsManager::class.java)).setMode(
            AppOpsManager.OPSTR_GET_USAGE_STATS, app.applicationInfo.uid, app.packageName, mode,
        )
    }

    private fun install(packageName: String, label: String) {
        shadowOf(app.packageManager).installPackage(PackageInfo().apply {
            this.packageName = packageName
            applicationInfo = ApplicationInfo().apply {
                this.packageName = packageName
                nonLocalizedLabel = label
                icon = android.R.drawable.ic_menu_edit
            }
        })
    }

    private fun event(type: Int, packageName: String, at: Long) {
        shadowOf(app.getSystemService(UsageStatsManager::class.java)).addEvent(EventBuilder.buildEvent()
            .setEventType(type).setPackage(packageName).setClass("Main").setTimeStamp(at).build())
    }

    @Implements(UsageStatsManager::class)
    class FailingUsageStats {
        @Implementation fun queryEvents(beginTime: Long, endTime: Long): UsageEvents? = throw SecurityException("Revoked")
    }
}
