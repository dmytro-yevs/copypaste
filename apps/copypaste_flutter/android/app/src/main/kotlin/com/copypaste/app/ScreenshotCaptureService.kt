package com.copypaste.app

import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.IBinder
import androidx.core.content.ContextCompat

class ScreenshotCaptureService : Service() {
    private var monitor: ScreenshotCaptureMonitor? = null
    override fun onBind(intent: Intent?): IBinder? = null
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (!eligible(this)) { stopSelf(startId); return START_NOT_STICKY }
        MainActivity.ensureRuntime(applicationContext)
        BackgroundActivityNotification.start(this)
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
