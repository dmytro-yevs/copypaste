package com.copypaste.android

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat

object NotificationHelper {
    private const val CHANNEL_SENSITIVE = "copypaste_sensitive"
    private const val CHANNEL_SYNC = "copypaste_sync"

    fun createChannels(context: Context) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val manager = context.getSystemService(NotificationManager::class.java)

            manager.createNotificationChannel(
                NotificationChannel(
                    CHANNEL_SENSITIVE,
                    "Sensitive Data",
                    NotificationManager.IMPORTANCE_HIGH
                ).apply {
                    description = "Alerts when sensitive data (API keys, passwords) is detected"
                }
            )

            manager.createNotificationChannel(
                NotificationChannel(
                    CHANNEL_SYNC,
                    "Sync Status",
                    NotificationManager.IMPORTANCE_LOW
                ).apply {
                    description = "Relay sync status notifications"
                }
            )
        }
    }

    /**
     * Post a one-shot "sync disabled" notification when the native encryption
     * library is unavailable.
     *
     * This is a SECURITY sentinel: the app must not silently downgrade to an
     * AES-GCM fallback (which produces items peers cannot decrypt) and must
     * instead make the failure visible. The notification fires at most once per
     * session because [ClipboardRepository] gates on
     * [nativeUnavailableNotified] before calling this.
     */
    fun notifyNativeUnavailable(context: Context, id: Int = 1002) {
        val notification = NotificationCompat.Builder(context, CHANNEL_SYNC)
            .setSmallIcon(android.R.drawable.ic_dialog_alert)
            .setContentTitle("CopyPaste: sync disabled")
            .setContentText(
                "The encryption library failed to load. " +
                    "Items will NOT be saved until the app is restarted."
            )
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setAutoCancel(true)
            .build()

        if (!NotificationManagerCompat.from(context).areNotificationsEnabled()) {
            Log.w(TAG, "notifyNativeUnavailable: notifications disabled — skipping")
            return
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS)
                    != PackageManager.PERMISSION_GRANTED) {
                Log.w(TAG, "notifyNativeUnavailable: POST_NOTIFICATIONS not granted — skipping")
                return
            }
        }
        try {
            NotificationManagerCompat.from(context).notify(id, notification)
        } catch (e: SecurityException) {
            Log.w(
                TAG,
                "notifyNativeUnavailable: notify() blocked by SecurityException: ${e.message}"
            )
        }
    }

    fun notifySensitiveDetected(context: Context, id: Int = 1001) {
        val notification = NotificationCompat.Builder(context, CHANNEL_SENSITIVE)
            .setSmallIcon(android.R.drawable.ic_dialog_alert)
            .setContentTitle("Sensitive data detected")
            .setContentText("An item with a secret key or credential was detected and not stored.")
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setAutoCancel(true)
            .build()

        // [P1] API 33+ (TIRAMISU) requires POST_NOTIFICATIONS permission at runtime.
        // NotificationManagerCompat.notify() throws SecurityException if the permission
        // was granted then revoked. Guard with both an areNotificationsEnabled() check
        // (covers all API levels) and a belt-and-suspenders try/catch for the
        // SecurityException path on API 33+ where revocation can race with notify().
        if (!NotificationManagerCompat.from(context).areNotificationsEnabled()) {
            Log.d(TAG, "notifySensitiveDetected: notifications disabled — skipping")
            return
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS)
                    != PackageManager.PERMISSION_GRANTED) {
                Log.d(TAG, "notifySensitiveDetected: POST_NOTIFICATIONS not granted — skipping")
                return
            }
        }
        try {
            NotificationManagerCompat.from(context).notify(id, notification)
        } catch (e: SecurityException) {
            Log.w(TAG, "notifySensitiveDetected: notify() blocked by SecurityException — " +
                "POST_NOTIFICATIONS was revoked concurrently: ${e.message}")
        }
    }

    private const val TAG = "NotificationHelper"
}
