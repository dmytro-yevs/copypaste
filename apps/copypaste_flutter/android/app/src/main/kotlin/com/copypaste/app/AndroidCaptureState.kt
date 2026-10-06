package com.copypaste.app

import android.Manifest
import android.app.AppOpsManager
import android.app.usage.UsageStatsManager
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.PowerManager
import android.provider.Settings
import androidx.core.content.ContextCompat

internal object AndroidCaptureState {
    // Public checkOpNoThrow accepts the stable operation names, while the SDK
    // constants for these two operations remain hidden from application code.
    private const val runInBackgroundOp = "android:run_in_background"
    private const val runAnyInBackgroundOp = "android:run_any_in_background"
    private const val preferences = "android-capture"
    private const val captureEnabledKey = "capture-enabled"
    private const val foregroundCaptureEnabledKey = "foreground-capture-enabled"
    private const val lastBackgroundCaptureAtKey = "last-background-capture-at"

    fun privilegedGrants(context: Context): Boolean =
        ContextCompat.checkSelfPermission(context, Manifest.permission.READ_LOGS) ==
            PackageManager.PERMISSION_GRANTED &&
            Settings.canDrawOverlays(context) &&
            backgroundAppOpsAllowed(context) &&
            activeForBackgroundWork(context)

    fun notificationGranted(context: Context): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) ==
            PackageManager.PERMISSION_GRANTED

    fun batteryExempt(context: Context): Boolean =
        context.getSystemService(PowerManager::class.java)
            ?.isIgnoringBatteryOptimizations(context.packageName) == true

    fun captureEnabled(context: Context): Boolean =
        context.getSharedPreferences(preferences, Context.MODE_PRIVATE)
            .getBoolean(captureEnabledKey, false)

    fun setCaptureEnabled(context: Context, enabled: Boolean) {
        context.getSharedPreferences(preferences, Context.MODE_PRIVATE)
            .edit()
            .putBoolean(captureEnabledKey, enabled)
            .apply()
    }

    fun foregroundCaptureEnabled(context: Context): Boolean =
        context.getSharedPreferences(preferences, Context.MODE_PRIVATE)
            .getBoolean(foregroundCaptureEnabledKey, false)

    fun setForegroundCaptureEnabled(context: Context, enabled: Boolean) {
        context.getSharedPreferences(preferences, Context.MODE_PRIVATE)
            .edit()
            .putBoolean(foregroundCaptureEnabledKey, enabled)
            .apply()
    }

    fun lastCaptureAt(context: Context): Long =
        context.getSharedPreferences(preferences, Context.MODE_PRIVATE)
            .getLong(lastBackgroundCaptureAtKey, 0L)

    fun recordBackgroundCapture(context: Context, at: Long = System.currentTimeMillis()) {
        context.getSharedPreferences(preferences, Context.MODE_PRIVATE)
            .edit()
            .putLong(lastBackgroundCaptureAtKey, at)
            .apply()
    }

    private fun backgroundAppOpsAllowed(context: Context): Boolean {
        val manager = context.getSystemService(AppOpsManager::class.java) ?: return false
        fun allowed(op: String): Boolean = runCatching {
            manager.checkOpNoThrow(op, context.applicationInfo.uid, context.packageName) ==
                AppOpsManager.MODE_ALLOWED
        }.getOrDefault(false)
        return allowed(runInBackgroundOp) &&
            (Build.VERSION.SDK_INT < Build.VERSION_CODES.O ||
                allowed(runAnyInBackgroundOp))
    }

    private fun activeForBackgroundWork(context: Context): Boolean {
        val manager = context.getSystemService(UsageStatsManager::class.java) ?: return false
        return runCatching {
            !manager.isAppInactive(context.packageName) &&
                (Build.VERSION.SDK_INT < Build.VERSION_CODES.P ||
                    // Exempted apps have a lower bucket and are also unrestricted.
                    manager.appStandbyBucket <= UsageStatsManager.STANDBY_BUCKET_ACTIVE)
        }.getOrDefault(false)
    }
}
