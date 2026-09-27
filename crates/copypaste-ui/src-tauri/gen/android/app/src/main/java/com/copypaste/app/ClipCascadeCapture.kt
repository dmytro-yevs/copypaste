package com.copypaste.app

import android.content.ComponentName
import android.content.Context
import android.content.ServiceConnection
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.provider.Settings
import android.util.Log
import rikka.shizuku.Shizuku

object ClipCascadeCapture {
    private const val TAG = "CopyPasteClipCascade"
    private const val CONNECT_TIMEOUT_MS = 3_000L
    private val main = Handler(Looper.getMainLooper())

    @Volatile
    private var session: Session? = null

    @Volatile
    private var appContext: Context? = null

    fun markSetupComplete(context: Context) = Unit

    fun isSetupComplete(context: Context): Boolean =
        hasRuntimePermissions(context) && ShizukuClipboard.isRunning() && ShizukuClipboard.hasPermission()

    fun hasRuntimePermissions(context: Context): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.M || Settings.canDrawOverlays(context)

    fun isListening(): Boolean = session?.listening == true

    @Synchronized
    fun arm(context: Context, onStarted: (Boolean) -> Unit, onLost: () -> Unit): Boolean {
        if (!isSetupComplete(context)) return false
        appContext = context.applicationContext
        session?.let {
            if (it.listening || it.connecting) {
                if (it.listening) onStarted(true)
                return true
            }
        }

        val next = Session(onStarted, onLost)
        session = next
        if (!main.postDelayed(next.timeout, CONNECT_TIMEOUT_MS)) {
            next.finish(false)
            return false
        }
        try {
            Shizuku.bindUserService(serviceArgs(), next)
        } catch (_: RuntimeException) {
            next.finish(false)
            return false
        }
        return true
    }

    @Synchronized
    fun disarm() {
        session?.stop(expected = true)
    }

    private fun serviceArgs() = Shizuku.UserServiceArgs(
        ComponentName(BuildConfig.APPLICATION_ID, ShizukuCaptureService::class.java.name),
    )
        .daemon(false)
        .tag("copypaste-clipboard-capture")
        .processNameSuffix("clipboard-capture")
        .debuggable(BuildConfig.DEBUG)
        .version(BuildConfig.VERSION_CODE)

    private class Session(
        private val onStarted: (Boolean) -> Unit,
        private val onLost: () -> Unit,
    ) : ServiceConnection {
        @Volatile var connecting = true
        @Volatile var listening = false
        private var expectedStop = false
        private var service: IShizukuCaptureService? = null
        private val callback = object : IClipCascadeCaptureListener.Stub() {
            override fun onClipboardAccess() {
                if (!listening) return
                val app = ClipCascadeCapture.appContext ?: return
                ClipCascadeCapture.main.post {
                    try {
                        app.startActivity(ClipboardFloatingActivity.intent(app))
                    } catch (error: Exception) {
                        Log.w(ClipCascadeCapture.TAG, "floating capture activity launch failed", error)
                    }
                }
            }

            override fun onCaptureStopped() = finish(false)
        }

        val timeout = Runnable { finish(false) }

        override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
            if (!connecting) return
            val capture = binder?.let(IShizukuCaptureService.Stub::asInterface)
            val started = try {
                binder?.pingBinder() == true && capture?.start(callback) == true
            } catch (_: Exception) {
                false
            }
            if (!started) {
                finish(false)
                return
            }
            service = capture
            connecting = false
            listening = true
            ClipCascadeCapture.main.removeCallbacks(timeout)
            ClipCascadeCapture.main.post { onStarted(true) }
        }

        override fun onServiceDisconnected(name: ComponentName?) = finish(false)
        override fun onNullBinding(name: ComponentName?) = finish(false)
        override fun onBindingDied(name: ComponentName?) = finish(false)

        fun stop(expected: Boolean) {
            expectedStop = expected
            try {
                service?.stop()
            } catch (_: Exception) {
            }
            finish(false)
        }

        @Synchronized
        fun finish(started: Boolean) {
            if (!connecting && !listening) return
            val wasListening = listening
            connecting = false
            listening = false
            ClipCascadeCapture.main.removeCallbacks(timeout)
            if (ClipCascadeCapture.session === this) ClipCascadeCapture.session = null
            try {
                Shizuku.unbindUserService(ClipCascadeCapture.serviceArgs(), this, true)
            } catch (_: RuntimeException) {
            } finally {
                try {
                    Shizuku.unbindUserService(ClipCascadeCapture.serviceArgs(), this, false)
                } catch (_: RuntimeException) {
                }
            }
            if (wasListening && !expectedStop) {
                ClipCascadeCapture.main.post(onLost)
            } else if (!wasListening) {
                ClipCascadeCapture.main.post { onStarted(started) }
            }
        }
    }

    init {
        Shizuku.addBinderDeadListener { session?.finish(false) }
    }
}
