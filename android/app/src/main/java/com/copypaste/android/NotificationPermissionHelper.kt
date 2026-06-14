package com.copypaste.android

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.core.content.ContextCompat

/**
 * CopyPaste-l080: shared logic for the POST_NOTIFICATIONS runtime permission and
 * the permanent-denial (Android 13+ caps the dialog after 2 denials) fallback to
 * the system app-notification-settings screen.
 *
 * Both [OnboardingActivity] and [PermissionsSettingsActivity] previously called
 * `notifLauncher.launch(POST_NOTIFICATIONS)` directly with no permanent-denial path,
 * so once a user denied twice every subsequent "Grant" tap was a silent no-op.
 */
object NotificationPermissionHelper {

    private const val PREFS = "copypaste_perm"
    private const val KEY_NOTIF_REQUESTED = "post_notifications_requested"
    private const val KEY_CAMERA_REQUESTED = "camera_requested"

    /** Pre-Tiramisu the permission is granted at install time. */
    fun isGranted(context: Context): Boolean {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return true
        return ContextCompat.checkSelfPermission(
            context, Manifest.permission.POST_NOTIFICATIONS,
        ) == PackageManager.PERMISSION_GRANTED
    }

    /** Record that the runtime dialog has been launched at least once. */
    fun markRequested(context: Context) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit().putBoolean(KEY_NOTIF_REQUESTED, true).apply()
    }

    private fun wasRequested(context: Context): Boolean =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .getBoolean(KEY_NOTIF_REQUESTED, false)

    /**
     * True when POST_NOTIFICATIONS is permanently denied: the permission is not
     * granted, the runtime dialog has been shown before, and the OS now refuses
     * to show the rationale (i.e. it will no longer present the dialog). In that
     * state a `launch(POST_NOTIFICATIONS)` is a silent no-op and the caller must
     * route the user to system Settings instead via [appNotificationSettingsIntents].
     *
     * On the FIRST request `shouldShowRequestPermissionRationale` is also false,
     * which is why we additionally require [wasRequested].
     */
    fun isPermanentlyDenied(activity: Activity): Boolean {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return false
        if (isGranted(activity)) return false
        if (!wasRequested(activity)) return false
        return !activity.shouldShowRequestPermissionRationale(Manifest.permission.POST_NOTIFICATIONS)
    }

    /**
     * Ordered list of Settings intents that lead the user to where they can flip
     * notifications back on: the app-notification-settings screen first, then the
     * generic app-details screen as a fallback for OEMs that don't resolve the
     * former. The caller launches the first that resolves.
     */
    fun appNotificationSettingsIntents(context: Context): List<Intent> {
        val intents = mutableListOf<Intent>()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            intents += Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).apply {
                putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName)
            }
        }
        intents += Intent(
            Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
            Uri.fromParts("package", context.packageName, null),
        )
        return intents
    }

    /**
     * Build the app-details Settings intents for any permission (used by the
     * CAMERA permanent-denial path). The app-details screen is the only reliable
     * deep-link for runtime permissions other than notifications.
     */
    fun appDetailsSettingsIntents(context: Context): List<Intent> = listOf(
        Intent(
            Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
            Uri.fromParts("package", context.packageName, null),
        ),
    )

    // ── CAMERA (CopyPaste-l080) ──────────────────────────────────────────────

    fun isCameraGranted(context: Context): Boolean =
        ContextCompat.checkSelfPermission(
            context, Manifest.permission.CAMERA,
        ) == PackageManager.PERMISSION_GRANTED

    fun markCameraRequested(context: Context) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit().putBoolean(KEY_CAMERA_REQUESTED, true).apply()
    }

    private fun wasCameraRequested(context: Context): Boolean =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .getBoolean(KEY_CAMERA_REQUESTED, false)

    /**
     * True when CAMERA is permanently denied: not granted, the dialog has been
     * shown before, and the OS now refuses to show the rationale. In that state a
     * `launch(CAMERA)` is a silent no-op; the caller must deep-link to app-details
     * Settings instead. Like notifications, on the FIRST request the rationale flag
     * is also false, so [wasCameraRequested] disambiguates.
     */
    fun isCameraPermanentlyDenied(activity: Activity): Boolean {
        if (isCameraGranted(activity)) return false
        if (!wasCameraRequested(activity)) return false
        return !activity.shouldShowRequestPermissionRationale(Manifest.permission.CAMERA)
    }

    /** Launch the first resolvable intent from [candidates]; returns true if one launched. */
    fun launchFirstResolvable(context: Context, candidates: List<Intent>): Boolean {
        for (intent in candidates) {
            try {
                context.startActivity(intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                return true
            } catch (_: Exception) {
                // try next
            }
        }
        return false
    }
}
