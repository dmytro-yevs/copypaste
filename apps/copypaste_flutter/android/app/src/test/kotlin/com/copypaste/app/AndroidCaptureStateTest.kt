package com.copypaste.app

import android.Manifest
import android.app.AppOpsManager
import android.app.Application
import android.app.usage.UsageStatsManager
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowSettings

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class AndroidCaptureStateTest {
    private lateinit var application: Application
    private lateinit var usage: UsageStatsManager

    @Before
    fun setUp() {
        application = RuntimeEnvironment.getApplication()
        usage = application.getSystemService(UsageStatsManager::class.java)
        shadowOf(application).grantPermissions(Manifest.permission.READ_LOGS)
        ShadowSettings.setCanDrawOverlays(true)
        val appOps = shadowOf(application.getSystemService(AppOpsManager::class.java))
        for (op in listOf("android:run_in_background", "android:run_any_in_background")) {
            appOps.setMode(op, application.applicationInfo.uid, application.packageName, AppOpsManager.MODE_ALLOWED)
        }
    }

    @Test
    fun exemptedAndActiveAppsAcceptCaptureGrants() {
        // Android's exempted bucket is hidden from the public SDK.
        for (bucket in listOf(5, UsageStatsManager.STANDBY_BUCKET_ACTIVE)) {
            shadowOf(usage).setCurrentAppStandbyBucket(bucket)
            assertTrue("Unrestricted standby bucket $bucket", AndroidCaptureState.privilegedGrants(application))
        }
    }

    @Test
    fun restrictedAppsRejectCaptureGrants() {
        for (bucket in listOf(
            UsageStatsManager.STANDBY_BUCKET_WORKING_SET,
            UsageStatsManager.STANDBY_BUCKET_FREQUENT,
            UsageStatsManager.STANDBY_BUCKET_RARE,
            UsageStatsManager.STANDBY_BUCKET_RESTRICTED,
        )) {
            shadowOf(usage).setCurrentAppStandbyBucket(bucket)
            assertFalse("Restricted standby bucket $bucket", AndroidCaptureState.privilegedGrants(application))
        }
    }

    @Test
    fun exemptedAppsStillRequireReadLogsPermission() {
        shadowOf(usage).setCurrentAppStandbyBucket(5)
        shadowOf(application).denyPermissions(Manifest.permission.READ_LOGS)
        assertFalse(AndroidCaptureState.privilegedGrants(application))
    }
}
