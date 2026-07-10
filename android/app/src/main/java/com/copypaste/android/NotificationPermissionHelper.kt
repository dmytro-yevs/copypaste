package com.copypaste.android

import android.Manifest
import android.annotation.SuppressLint
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
     * Pure POST_NOTIFICATIONS state machine (S10 Wave A / CopyPaste-myh8.10):
     * no Context/Activity, so it is unit-testable in the :app JVM test module.
     * DI-seam parameters mirror the Activity-bound checks in [notificationPermissionStatus].
     *
     * On the FIRST request `shouldShowRationale` is also false, which is why
     * PERMANENTLY_DENIED additionally requires [wasRequested] — otherwise the
     * very first (never-yet-shown) request would be misclassified as permanent.
     */
    fun notificationStatus(
        sdkInt: Int,
        isGranted: Boolean,
        wasRequested: Boolean,
        shouldShowRationale: Boolean,
    ): PermissionStatus {
        if (sdkInt < Build.VERSION_CODES.TIRAMISU) return PermissionStatus.NOT_APPLICABLE
        if (isGranted) return PermissionStatus.GRANTED
        if (!wasRequested) return PermissionStatus.DENIED
        return if (shouldShowRationale) PermissionStatus.DENIED else PermissionStatus.PERMANENTLY_DENIED
    }

    /**
     * Activity-bound wrapper around [notificationStatus] for real call sites.
     *
     * [Manifest.permission.POST_NOTIFICATIONS] is referenced unconditionally
     * here (its field requires API 33) even though [notificationStatus] SDK-
     * gates the actual verdict below TIRAMISU — the permission string constant
     * is compile-time-inlined and safe to reference on any minSdk 26 device;
     * [SuppressLint] documents that this is a deliberate, verified-safe read,
     * not a missing guard.
     */
    @SuppressLint("InlinedApi")
    fun notificationPermissionStatus(activity: Activity): PermissionStatus = notificationStatus(
        sdkInt = Build.VERSION.SDK_INT,
        isGranted = isGranted(activity),
        wasRequested = wasRequested(activity),
        shouldShowRationale = activity.shouldShowRequestPermissionRationale(Manifest.permission.POST_NOTIFICATIONS),
    )

    /**
     * True when POST_NOTIFICATIONS is permanently denied: the permission is not
     * granted, the runtime dialog has been shown before, and the OS now refuses
     * to show the rationale (i.e. it will no longer present the dialog). In that
     * state a `launch(POST_NOTIFICATIONS)` is a silent no-op and the caller must
     * route the user to system Settings instead via [appNotificationSettingsIntents].
     */
    fun isPermanentlyDenied(activity: Activity): Boolean =
        notificationPermissionStatus(activity) == PermissionStatus.PERMANENTLY_DENIED

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
     * Pure CAMERA state machine — same shape as [notificationStatus] but without
     * SDK gating (CAMERA is a runtime permission on every supported API level).
     */
    fun cameraStatus(
        isGranted: Boolean,
        wasRequested: Boolean,
        shouldShowRationale: Boolean,
    ): PermissionStatus {
        if (isGranted) return PermissionStatus.GRANTED
        if (!wasRequested) return PermissionStatus.DENIED
        return if (shouldShowRationale) PermissionStatus.DENIED else PermissionStatus.PERMANENTLY_DENIED
    }

    /** Activity-bound wrapper around [cameraStatus] for real call sites. */
    fun cameraPermissionStatus(activity: Activity): PermissionStatus = cameraStatus(
        isGranted = isCameraGranted(activity),
        wasRequested = wasCameraRequested(activity),
        shouldShowRationale = activity.shouldShowRequestPermissionRationale(Manifest.permission.CAMERA),
    )

    /**
     * True when CAMERA is permanently denied: not granted, the dialog has been
     * shown before, and the OS now refuses to show the rationale. In that state a
     * `launch(CAMERA)` is a silent no-op; the caller must deep-link to app-details
     * Settings instead.
     */
    fun isCameraPermanentlyDenied(activity: Activity): Boolean =
        cameraPermissionStatus(activity) == PermissionStatus.PERMANENTLY_DENIED

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
