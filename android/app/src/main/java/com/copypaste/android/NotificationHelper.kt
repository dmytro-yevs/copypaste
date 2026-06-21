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
    // ANDO-2: CHANNEL_SENSITIVE + notifySensitiveDetected removed — no caller exists anywhere
    // in the codebase. The sensitive-detection path silently drops items rather than notifying.
    // CHANNEL_SYNC is kept because notifyNativeUnavailable (CHANNEL_SYNC) is called from
    // ClipboardRepository (6 call-sites) as a security sentinel for native library failures.
    private const val CHANNEL_SYNC = "copypaste_sync"

    fun createChannels(context: Context) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val manager = context.getSystemService(NotificationManager::class.java)

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
        // ANDO-7: use app icon (monochrome foreground layer) rather than generic system drawable.
        // ic_launcher_foreground is an alpha-channel PNG produced from the adaptive-icon foreground —
        // it is white-on-transparent on API 26+ where adaptive icons are used as notification icons.
        val notification = NotificationCompat.Builder(context, CHANNEL_SYNC)
            .setSmallIcon(R.mipmap.ic_launcher_foreground)
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

    private const val TAG = "NotificationHelper"
}
