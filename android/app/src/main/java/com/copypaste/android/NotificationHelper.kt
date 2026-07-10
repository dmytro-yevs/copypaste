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
                    context.getString(R.string.notif_channel_sync_name),
                    NotificationManager.IMPORTANCE_LOW
                ).apply {
                    description = context.getString(R.string.notif_channel_sync_description)
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
        // ANDO-7: use a dedicated alpha-only status-bar icon, not the full-color adaptive
        // foreground — the OS masks smallIcon by alpha, so a colored source renders as a blob.
        val notification = NotificationCompat.Builder(context, CHANNEL_SYNC)
            .setSmallIcon(R.drawable.ic_stat_notify)
            .setContentTitle(context.getString(R.string.notif_native_unavailable_title))
            .setContentText(context.getString(R.string.notif_native_unavailable_content))
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
