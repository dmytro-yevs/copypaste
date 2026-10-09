package com.copypaste.app

import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.IBinder
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
        BackgroundActivityNotification.start(this)
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

    companion object {
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
