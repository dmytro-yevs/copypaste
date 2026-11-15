package com.copypaste.app

import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.IBinder
import androidx.core.content.ContextCompat
import java.util.concurrent.atomic.AtomicBoolean

class ClipboardCaptureService : Service() {
    private var captureHostId = 0L
    private var monitor: BackgroundCaptureMonitor? = null
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        AndroidClipboardCache.scheduleCleanup(applicationContext)
        MainActivity.ensureRuntime(applicationContext)
        running.set(true)
        NativeRuntimeCapture.setCaptureRunning(true)
        AndroidCaptureSignals.changed()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val selected = monitorFor(AndroidCaptureState.captureMethod(this))
        if (monitor != null && monitor !== selected) monitor?.stop(expectedHost = captureHostId)
        monitor = selected
        activeMonitor = selected
        if (!AndroidCaptureState.captureEnabled(this) ||
            !AndroidCaptureState.backgroundCaptureReady(this) ||
            !AndroidCaptureState.notificationGranted(this)
        ) {
            if (AndroidCaptureState.captureEnabled(this)) RuntimeLogEvent.CAPTURE_ACCESS_MISSING.log()
            selected.stop()
            stopSelf(startId)
            return START_NOT_STICKY
        }
        BackgroundActivityNotification.start(this)
        if (!selected.start(
                this,
                onStarted = { started ->
                    AndroidCaptureSignals.changed()
                    if (started) {
                        RuntimeLogEvent.CAPTURE_SERVICE_STARTED.log()
                    } else {
                        RuntimeLogEvent.CAPTURE_LOST.log()
                        stopCapture(this)
                    }
                },
                onLost = {
                    RuntimeLogEvent.CAPTURE_LOST.log()
                    stopCapture(this)
                },
            )
        ) {
            RuntimeLogEvent.CAPTURE_ACCESS_MISSING.log()
            selected.stop()
            stopSelf(startId)
        }
        captureHostId = selected.hostId()
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        val current = monitor
        current?.stop(expectedHost = captureHostId)
        val listening = current?.isListening() == true
        if (!listening) RuntimeLogEvent.CAPTURE_SERVICE_STOPPED.log()
        running.set(listening)
        NativeRuntimeCapture.setCaptureRunning(listening)
        AndroidCaptureSignals.changed()
        BackgroundActivityNotification.refresh(this)
        super.onDestroy()
    }

    companion object {
        private val running = AtomicBoolean(false)
        @Volatile private var activeMonitor: BackgroundCaptureMonitor? = null

        private fun monitorFor(method: BackgroundCaptureMethod): BackgroundCaptureMonitor = when (method) {
            BackgroundCaptureMethod.LOG_TRIGGER -> BackgroundClipboardMonitor
            BackgroundCaptureMethod.POLLING -> ClipboardPollingMonitor
        }

        fun isRunning(): Boolean = running.get() && activeMonitor?.isListening() == true

        fun startCapture(context: Context): Boolean {
            if (!AndroidCaptureState.backgroundCaptureReady(context) ||
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
            if (AndroidCaptureState.captureEnabled(context) && !isRunning()) startCapture(context)
        }

        /** Switches the method; enabled capture restarts with it once its access is ready. */
        internal fun selectMethod(context: Context, method: BackgroundCaptureMethod, completion: (Boolean) -> Unit) {
            if (AndroidCaptureState.captureMethod(context) == method) { completion(true); return }
            val enabled = AndroidCaptureState.captureEnabled(context)
            stopCapture(context) { drained ->
                AndroidCaptureState.setCaptureMethod(context, method)
                RuntimeLogEvent.CAPTURE_METHOD_CHANGED.log()
                if (enabled) {
                    AndroidCaptureState.setCaptureEnabled(context, true)
                    startCapture(context)
                }
                completion(drained)
            }
        }

        fun stopCapture(context: Context, completion: (Boolean) -> Unit = {}) {
            AndroidCaptureState.setCaptureEnabled(context, false)
            (activeMonitor ?: BackgroundClipboardMonitor).stop(completion = completion)
            context.stopService(Intent(context, ClipboardCaptureService::class.java))
            running.set(false)
            NativeRuntimeCapture.setCaptureRunning(false)
        }
    }
}
