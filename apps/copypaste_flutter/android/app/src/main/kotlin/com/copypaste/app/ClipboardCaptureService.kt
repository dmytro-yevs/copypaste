package com.copypaste.app

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import java.util.concurrent.atomic.AtomicBoolean

class ClipboardCaptureService : Service() {
    private var captureHostId = 0L
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        running.set(true)
        NativeRuntimeCapture.setCaptureRunning(true)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (!AndroidCaptureState.captureEnabled(this) ||
            !AndroidCaptureState.privilegedGrants(this) ||
            !AndroidCaptureState.notificationGranted(this)
        ) {
            BackgroundClipboardMonitor.stop()
            stopSelf(startId)
            return START_NOT_STICKY
        }
        ensureNotificationChannel()
        startForeground(notificationId, notification())
        if (!BackgroundClipboardMonitor.start(
                this,
                onStarted = { started ->
                    if (!started) stopCapture(this)
                },
                onLost = { stopCapture(this) },
            )
        ) {
            BackgroundClipboardMonitor.stop()
            stopSelf(startId)
        }
        captureHostId = BackgroundClipboardMonitor.hostId()
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        BackgroundClipboardMonitor.stop(expectedHost = captureHostId)
        val listening = BackgroundClipboardMonitor.isListening()
        running.set(listening)
        NativeRuntimeCapture.setCaptureRunning(listening)
        super.onDestroy()
    }

    private fun ensureNotificationChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = getSystemService(NotificationManager::class.java) ?: return
        manager.createNotificationChannel(
            NotificationChannel(
                notificationChannel,
                "Clipboard capture",
                NotificationManager.IMPORTANCE_LOW,
            ).apply {
                description = "Keeps CopyPaste ready to save clipboard changes from other apps."
            },
        )
    }

    private fun notification() = NotificationCompat.Builder(this, notificationChannel)
        .setSmallIcon(R.mipmap.ic_launcher)
        .setContentTitle("CopyPaste is capturing")
        .setContentText("Clipboard changes from other apps are saved to your private history.")
        .setOngoing(true)
        .setOnlyAlertOnce(true)
        .setContentIntent(
            PendingIntent.getActivity(
                this,
                0,
                Intent(this, MainActivity::class.java),
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            ),
        )
        .build()

    companion object {
        private const val notificationChannel = "clipboard-capture"
        private const val notificationId = 2207
        private val running = AtomicBoolean(false)

        fun isRunning(): Boolean = running.get() && BackgroundClipboardMonitor.isListening()

        fun startCapture(context: Context): Boolean {
            if (!AndroidCaptureState.privilegedGrants(context) ||
                !AndroidCaptureState.notificationGranted(context)
            ) {
                return false
            }
            AndroidCaptureState.setCaptureEnabled(context, true)
            return try {
                ContextCompat.startForegroundService(
                    context,
                    Intent(context, ClipboardCaptureService::class.java),
                )
                true
            } catch (_: RuntimeException) {
                AndroidCaptureState.setCaptureEnabled(context, false)
                false
            }
        }

        fun restoreIfEnabled(context: Context) {
            if (AndroidCaptureState.captureEnabled(context)) startCapture(context)
        }

        fun stopCapture(context: Context, completion: (Boolean) -> Unit = {}) {
            AndroidCaptureState.setCaptureEnabled(context, false)
            BackgroundClipboardMonitor.stop(completion = completion)
            context.stopService(Intent(context, ClipboardCaptureService::class.java))
            running.set(false)
            NativeRuntimeCapture.setCaptureRunning(false)
        }
    }
}
