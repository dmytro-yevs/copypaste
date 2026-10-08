package com.copypaste.app

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat

class ScreenshotCaptureService : Service() {
    private var monitor: ScreenshotCaptureMonitor? = null
    override fun onBind(intent: Intent?): IBinder? = null
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (!eligible(this)) { stopSelf(startId); return START_NOT_STICKY }
        MainActivity.ensureRuntime(applicationContext)
        if (Build.VERSION.SDK_INT >= 26) getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel(channel, "Screenshot capture", NotificationManager.IMPORTANCE_LOW),
        )
        startForeground(2208, NotificationCompat.Builder(this, channel)
            .setSmallIcon(R.mipmap.ic_launcher)
            .setContentTitle("CopyPaste is saving screenshots")
            .setContentText("New screenshots are saved to your private history.")
            .setOngoing(true).setOnlyAlertOnce(true)
            .setContentIntent(PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java),
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)).build())
        if (monitor == null) {
            monitor = ScreenshotCaptureMonitor(applicationContext)
            if (monitor?.start() != true) { stopSelf(startId); return START_NOT_STICKY }
        }
        monitor?.refresh()
        active = this
        return START_STICKY
    }
    override fun onDestroy() {
        monitor?.close()
        monitor = null
        if (active === this) active = null
        super.onDestroy()
    }
    companion object {
        private const val channel = "screenshot-capture"
        @Volatile private var active: ScreenshotCaptureService? = null
        fun isRunning(): Boolean = active != null
        private fun eligible(context: Context): Boolean = ScreenshotCaptureState.enabled(context) &&
            ScreenshotCaptureState.mediaGranted(context) && AndroidCaptureState.notificationGranted(context)
        fun restoreIfEnabled(context: Context): Boolean {
            if (!eligible(context)) {
                if (active != null) stop(context) {}
                return false
            }
            return runCatching {
                ContextCompat.startForegroundService(context, Intent(context, ScreenshotCaptureService::class.java))
                true
            }.getOrDefault(false)
        }
        fun stop(context: Context, completion: (Boolean) -> Unit) {
            val service = active
            active = null
            val monitor = service?.monitor
            service?.monitor = null
            context.stopService(Intent(context, ScreenshotCaptureService::class.java))
            monitor?.close(completion) ?: completion(true)
        }
    }
}

class ScreenshotCaptureBootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action == Intent.ACTION_BOOT_COMPLETED || intent.action == Intent.ACTION_MY_PACKAGE_REPLACED) {
            ScreenshotCaptureService.restoreIfEnabled(context)
        }
    }
}
