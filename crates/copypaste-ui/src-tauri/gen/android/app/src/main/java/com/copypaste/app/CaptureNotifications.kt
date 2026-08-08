package com.copypaste.app

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat

/**
 * The notification that says background capture stopped.
 *
 * Shizuku dies on every reboot — its own documentation says the startup steps
 * must be repeated after each restart — so this is a routine part of the
 * product, not an error path. It exists because the alternative is a clipboard
 * manager that quietly saves nothing and a user who finds out when they go
 * looking for something that is not there.
 *
 * The wording comes from the Rust side (`capture::model::LOST_TITLE`), passed
 * down at arm time, so it can be posted by whichever half of the process is
 * still alive.
 */
object CaptureNotifications {
    private const val CHANNEL_STATUS = "capture-status"
    private const val CHANNEL_LOST = "capture-lost"
    const val ONGOING_ID = 1
    private const val LOST_ID = 2

    /** Lands on the rung 2 screen with the start step selected. */
    const val EXTRA_REARM = "com.copypaste.app.REARM"

    fun isPermissionGranted(context: Context): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            ContextCompat.checkSelfPermission(
                context,
                android.Manifest.permission.POST_NOTIFICATIONS,
            ) == android.content.pm.PackageManager.PERMISSION_GRANTED

    fun ensureChannels(context: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = context.getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(
                CHANNEL_STATUS,
                "Capture status",
                NotificationManager.IMPORTANCE_LOW,
            )
        )
        manager.createNotificationChannel(
            NotificationChannel(
                CHANNEL_LOST,
                "Capture stopped",
                // Default rather than low: this one is the whole point. A
                // silent notice about having silently stopped saving would be
                // the same failure one level up.
                NotificationManager.IMPORTANCE_DEFAULT,
            )
        )
    }

    /** Android 13+ blocks ordinary notifications until this runtime grant. */
    fun canPost(context: Context): Boolean =
        NotificationManagerCompat.from(context).areNotificationsEnabled() &&
            isPermissionGranted(context)

    fun ongoing(context: Context, text: String): Notification =
        builder(context, CHANNEL_STATUS)
            .setContentTitle("CopyPaste")
            .setContentText(text)
            .setSmallIcon(android.R.drawable.ic_menu_save)
            .setOngoing(true)
            .setContentIntent(open(context, rearm = false))
            .build()

    fun postLost(context: Context, title: String, body: String) {
        if (!canPost(context)) return
        ensureChannels(context)
        val notification = builder(context, CHANNEL_LOST)
            .setContentTitle(title)
            .setContentText(body)
            .setStyle(Notification.BigTextStyle().bigText(body))
            .setSmallIcon(android.R.drawable.stat_notify_error)
            .setAutoCancel(true)
            .setContentIntent(open(context, rearm = true))
            .build()
        context.getSystemService(NotificationManager::class.java)
            .notify(LOST_ID, notification)
    }

    private fun builder(context: Context, channel: String): Notification.Builder =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(context, channel)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(context)
        }

    /** One tap, straight to the step that fixes it. */
    private fun open(context: Context, rearm: Boolean): PendingIntent {
        val intent = Intent(context, MainActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
            .putExtra(EXTRA_REARM, rearm)
        return PendingIntent.getActivity(
            context,
            if (rearm) 1 else 0,
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
    }
}
