package com.copypaste.android

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Intent
import android.os.Build
import android.os.IBinder
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch

/**
 * Foreground service for clipboard monitoring on Android 8.0+ (API 26-28).
 * On Android 10+ (API 29+), clipboard access from background requires
 * the DEFAULT_INPUT_METHOD permission — not feasible for regular apps.
 * This service runs on API 26-28 where background clipboard access is allowed.
 */
class ClipboardService : Service() {

    private val scope = CoroutineScope(Dispatchers.IO)
    private lateinit var settings: Settings

    override fun onCreate() {
        super.onCreate()
        settings = Settings(this)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        startForeground(NOTIFICATION_ID, buildNotification())

        // Note: on API 29+, clipboard.primaryClip will be null from background
        // This service is only useful on API 26-28
        scope.launch {
            monitorClipboard()
        }

        return START_STICKY
    }

    private suspend fun monitorClipboard() {
        // TODO: poll clipboard and call UniFFI encrypt/store when .so lands
        // For now just runs as a persistent background service placeholder
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun buildNotification(): Notification {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID, "CopyPaste Service",
                NotificationManager.IMPORTANCE_LOW
            )
            getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
        }

        return android.app.Notification.Builder(this, CHANNEL_ID)
            .setContentTitle("CopyPaste")
            .setContentText("Monitoring clipboard...")
            .setSmallIcon(android.R.drawable.ic_menu_clipboard)
            .build()
    }

    companion object {
        private const val NOTIFICATION_ID = 1001
        private const val CHANNEL_ID = "copypaste_service"
    }
}
