package com.copypaste.app

import android.app.Application
import android.content.Intent
import android.content.pm.ActivityInfo
import android.content.pm.ApplicationInfo
import android.content.pm.PackageInfo
import android.content.pm.ResolveInfo
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
class ScreenshotSourceAppTest {
    private lateinit var app: Application

    @Before fun setup() { app = RuntimeEnvironment.getApplication() }

    @Test fun observedColorOsFilenamesResolveWithoutUsageAccess() {
        // Regression fixtures from a physical OPPO CPH2305 on Android 16.
        val fixtures = listOf(
            Triple("org.telegram.messenger", "Telegram", "Screenshot_2026-10-09-20-04-02-75_948cd9899890cbd5c2798760b2b95377.jpg"),
            Triple("com.anthropic.claude", "Claude", "Screenshot_2026-10-09-20-04-53-77_ae33c6f1cf6771e633bcb779de95c7e0.jpg"),
            Triple("com.vivaldi.browser", "Vivaldi", "Screenshot_2026-10-09-20-05-03-71_d365b52accad0f47adbc08c16219827d.jpg"),
            Triple("com.clipcascade", "ClipCascade", "Screenshot_2026-10-09-20-07-51-31_6c78667ca2d2c60cb40e19d783b314e9.jpg"),
            Triple("com.android.launcher", "Launcher", "Screenshot_2026-10-09-20-08-44-25_b783bf344239542886fee7b48fa4b892.jpg"),
        )
        for ((packageName, name, _) in fixtures) install(packageName, name,
            if (packageName == "com.android.launcher") Intent.CATEGORY_HOME else Intent.CATEGORY_LAUNCHER)
        for ((packageName, name, filename) in fixtures) {
            val source = ScreenshotSourceApps.resolve(app, filename)!!
            assertEquals(packageName, source.packageName)
            assertEquals(name, source.name)
            assertNotNull(source.icon)
        }
    }

    @Test fun repeatedLauncherActivitiesDoNotMakeThePackageAmbiguous() {
        install("org.telegram.messenger", "Telegram")
        install("org.telegram.messenger", "Telegram")
        assertEquals("org.telegram.messenger", ScreenshotSourceApps.resolve(app,
            "Screenshot_2026-10-09-20-04-02-75_948cd9899890cbd5c2798760b2b95377.jpg")?.packageName)
    }

    @Test fun unsupportedOrUnknownFilenamesReturnNoSourceInsteadOfGuessing() {
        install("org.telegram.messenger", "Telegram")
        for (filename in listOf("Screenshot.png", "", "IMG_948cd9899890cbd5c2798760b2b95377.jpg",
            "Screenshot_2026-10-09-20-04-02-75_00000000000000000000000000000000.jpg")) {
            assertNull(ScreenshotSourceApps.resolve(app, filename))
        }
    }

    @Test fun uninstalledSourceDoesNotPreventScreenshotCapture() {
        assertNull(ScreenshotSourceApps.resolve(app,
            "Screenshot_2026-10-09-20-04-02-75_948cd9899890cbd5c2798760b2b95377.jpg"))
        assertTrue(ScreenshotCaptureState.asMap(app)["enabled"]!!)
    }

    @Test fun uppercaseFilenameMetadataIsAccepted() {
        install("org.telegram.messenger", "Telegram")
        assertEquals("org.telegram.messenger", ScreenshotSourceApps.resolve(app,
            "SCREENSHOT_2026-10-09-20-04-02-75_948CD9899890CBD5C2798760B2B95377.JPG")?.packageName)
    }

    private fun install(packageName: String, label: String, category: String = Intent.CATEGORY_LAUNCHER) {
        val info = ApplicationInfo().apply {
            this.packageName = packageName
            nonLocalizedLabel = label
            icon = android.R.drawable.ic_menu_edit
        }
        shadowOf(app.packageManager).installPackage(PackageInfo().apply {
            this.packageName = packageName
            applicationInfo = info
        })
        shadowOf(app.packageManager).addResolveInfoForIntent(
            Intent(Intent.ACTION_MAIN).addCategory(category),
            ResolveInfo().apply { activityInfo = ActivityInfo().apply {
                this.packageName = packageName
                name = "$packageName.Main"
                applicationInfo = info
            } },
        )
    }
}
