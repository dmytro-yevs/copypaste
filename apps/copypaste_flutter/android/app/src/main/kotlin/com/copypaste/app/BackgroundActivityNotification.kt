package com.copypaste.app

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.os.Build
import androidx.core.app.NotificationCompat

/** One notification shared by every CopyPaste foreground service. */
internal object BackgroundActivityNotification {
    // Keep the clipboard channel and ID to preserve existing channel preferences.
    private const val channelId = "clipboard-capture"
    private const val notificationId = 2207

    fun start(service: Service) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            service.getSystemService(NotificationManager::class.java).createNotificationChannel(
                NotificationChannel(
                    channelId,
                    service.getString(R.string.background_channel_name),
                    NotificationManager.IMPORTANCE_LOW,
                ).apply {
                    description = service.getString(R.string.background_channel_description)
                    setSound(null, null)
                    enableVibration(false)
                },
            )
        }
        val notification = NotificationCompat.Builder(service, channelId)
            .setSmallIcon(R.mipmap.ic_launcher)
            .setContentTitle(service.getString(R.string.background_title))
            .setContentText(service.getString(R.string.background_text))
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setContentIntent(
                PendingIntent.getActivity(
                    service,
                    0,
                    Intent(service, MainActivity::class.java),
                    PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
                ),
            )
            .build()
        // Android retains this shared ID until the last foreground service stops.
        // Do not cancel it from an individual service's shutdown path.
        service.startForeground(notificationId, notification)
    }
}
