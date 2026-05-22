package com.copypaste.android

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat

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

    fun notifySensitiveDetected(context: Context, id: Int = 1001) {
        val notification = NotificationCompat.Builder(context, CHANNEL_SENSITIVE)
            .setSmallIcon(android.R.drawable.ic_dialog_alert)
            .setContentTitle("Sensitive data detected")
            .setContentText("An item with a secret key or credential was detected and not stored.")
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setAutoCancel(true)
            .build()

        NotificationManagerCompat.from(context).notify(id, notification)
    }
}
