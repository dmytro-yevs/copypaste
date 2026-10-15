package com.copypaste.app

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.content.ContextCompat

internal object ScreenshotCaptureState {
    private const val mediaPermissionAvailableKey = "media-permission-available"

    private fun preferences(context: Context) =
        context.getSharedPreferences("screenshot-capture", Context.MODE_PRIVATE)

    fun enabled(context: Context): Boolean = preferences(context).getBoolean("enabled", true)

    fun initialize(context: Context, now: Long = System.currentTimeMillis()) {
        val preferences = preferences(context)
        val editor = preferences.edit()
        if (!preferences.contains("since")) editor.putLong("since", now)
        if (!preferences.contains(mediaPermissionAvailableKey)) {
            // An older installation has no trustworthy permission boundary. Do
            // not turn its existing MediaStore backlog into "new" screenshots.
            if (preferences.contains("since")) editor.putLong("since", maxOf(since(context), now))
            editor.putBoolean(mediaPermissionAvailableKey, mediaGranted(context))
        }
        editor.commit()
    }

    fun since(context: Context): Long = preferences(context).getLong("since", 0L)

    fun setEnabled(context: Context, enabled: Boolean, now: Long = System.currentTimeMillis()) {
        val preferences = preferences(context)
        val editor = preferences.edit().putBoolean("enabled", enabled)
        if (enabled && !preferences.getBoolean("enabled", true)) editor.putLong("since", now)
        editor.commit()
    }

    fun skipThrough(context: Context, now: Long = System.currentTimeMillis()) {
        preferences(context).edit().putLong("since", maxOf(since(context), now)).commit()
    }

    fun paused(context: Context): Boolean = preferences(context).getBoolean("paused", false)

    fun pause(context: Context) {
        preferences(context).edit().putBoolean("paused", true).commit()
        skipThrough(context)
    }
    fun resume(context: Context) {
        if (preferences(context).getBoolean("paused", false)) {
            skipThrough(context)
            preferences(context).edit().putBoolean("paused", false).commit()
        }
    }

    fun permissionRequested(context: Context): Boolean = preferences(context).getBoolean("requested", false)
    fun markPermissionRequested(context: Context) { preferences(context).edit().putBoolean("requested", true).commit() }
    fun permissionAttempts(context: Context): Int = preferences(context).getInt("attempts", 0)
    fun recordPermissionAttempt(context: Context) {
        preferences(context).edit().putInt("attempts", permissionAttempts(context) + 1).commit()
    }

    fun mediaPermission(): String = if (Build.VERSION.SDK_INT >= 33) Manifest.permission.READ_MEDIA_IMAGES
        else Manifest.permission.READ_EXTERNAL_STORAGE

    fun mediaGranted(context: Context): Boolean =
        ContextCompat.checkSelfPermission(context, mediaPermission()) == PackageManager.PERMISSION_GRANTED

    /**
     * Records permission availability and starts a fresh screenshot window when
     * access returns. Screenshots created while photo access was denied must
     * never be imported after a later grant.
     */
    fun reconcileMediaPermission(context: Context, now: Long = System.currentTimeMillis()): Boolean {
        val preferences = preferences(context)
        val granted = mediaGranted(context)
        val observed = preferences.contains(mediaPermissionAvailableKey)
        val wasGranted = preferences.getBoolean(mediaPermissionAvailableKey, false)
        if (observed && granted == wasGranted) return granted
        val editor = preferences.edit().putBoolean(mediaPermissionAvailableKey, granted)
        if (granted) editor.putLong("since", maxOf(since(context), now))
        editor.commit()
        return granted
    }

    fun asMap(context: Context): Map<String, Boolean> = mapOf(
        "enabled" to enabled(context),
        "mediaGranted" to mediaGranted(context),
        "notificationGranted" to AndroidCaptureState.notificationGranted(context),
        "running" to ScreenshotCaptureService.isRunning(),
    )
}
