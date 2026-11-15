package com.copypaste.app

import android.app.KeyguardManager
import android.content.BroadcastReceiver
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.graphics.PixelFormat
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import android.view.Gravity
import android.view.View
import android.view.WindowManager
import androidx.core.content.ContextCompat

internal enum class PollingStep { PAUSE, READ }

/** Decides when a poll may take focus and whether the clipboard changed since the last one. */
internal class ClipboardPollingPolicy(
    lastSeenTimestamp: Long,
    val intervalMs: Long = DEFAULT_INTERVAL_MS,
) {
    private var lastSeen = lastSeenTimestamp

    init {
        require(intervalMs in MINIMUM_INTERVAL_MS..MAXIMUM_INTERVAL_MS)
    }

    /**
     * A dark or locked screen pauses polling until the screen broadcasts resume it;
     * a visible app pauses it until [ClipboardPollingMonitor.appBackgrounded].
     */
    fun step(interactive: Boolean, locked: Boolean, appVisible: Boolean): PollingStep = when {
        !interactive || locked || appVisible -> PollingStep.PAUSE
        else -> PollingStep.READ
    }

    /** True once per new clipboard description timestamp. */
    fun changed(timestamp: Long?): Boolean {
        if (timestamp == null || timestamp <= 0L || timestamp == lastSeen) return false
        lastSeen = timestamp
        return true
    }

    companion object {
        const val DEFAULT_INTERVAL_MS = 1_000L
        const val MINIMUM_INTERVAL_MS = 500L
        const val MAXIMUM_INTERVAL_MS = 10_000L
        const val FOCUS_TIMEOUT_MS = 300L
    }
}

/**
 * Background capture that needs only the overlay permission. A persistent 1px
 * overlay is made focusable for one clipboard description read per interval;
 * the full clip is read only when its timestamp changed. Main thread only.
 */
internal object ClipboardPollingMonitor : BackgroundCaptureMonitor {
    private val main = Handler(Looper.getMainLooper())
    private var app: Context? = null
    private var host: AndroidClipboardReader.Host? = null
    private var policy: ClipboardPollingPolicy? = null
    private var windowManager: WindowManager? = null
    private var overlay: FocusProbe? = null
    private var params: WindowManager.LayoutParams? = null
    private var screenReceiver: BroadcastReceiver? = null
    private var clipboardListener: ClipboardManager.OnPrimaryClipChangedListener? = null
    private var onLost: (() -> Unit)? = null
    private var awaitingFocus = false
    @Volatile private var listening = false
    private val tick = Runnable(::poll)
    private val focusTimeout = Runnable { if (awaitingFocus) release() }

    override fun hostId(): Long = host?.id ?: 0L

    override fun isListening(): Boolean = listening

    /** Resumes polling that a visible app paused. Main thread only. */
    fun appBackgrounded() {
        if (listening && !awaitingFocus) schedule()
    }

    override fun start(context: Context, onStarted: (Boolean) -> Unit, onLost: () -> Unit): Boolean {
        if (listening) {
            onStarted(true)
            return true
        }
        if (!AndroidCaptureState.overlayGranted(context)) return false
        stop()
        val opened = AndroidClipboardReader.startBackground() ?: return false
        val app = context.applicationContext
        this.app = app
        host = opened
        this.onLost = onLost
        try {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
                // Background clipboard reads are unrestricted before Android 10.
                listenDirectly(app, opened)
            } else {
                addOverlay(app)
                registerScreenReceiver(app)
                policy = ClipboardPollingPolicy(AndroidCaptureState.lastCaptureAt(app))
                schedule()
            }
        } catch (_: RuntimeException) {
            stop(expectedHost = opened.id)
            return false
        }
        listening = true
        RuntimeLogEvent.POLLING_STARTED.log()
        main.post { if (listening && host === opened) onStarted(true) }
        return true
    }

    override fun stop(expectedHost: Long?, completion: (Boolean) -> Unit) {
        if (Looper.myLooper() != Looper.getMainLooper()) {
            main.post { stop(expectedHost, completion) }
            return
        }
        if (expectedHost != null && hostId() != expectedHost) {
            completion(true)
            return
        }
        if (listening) RuntimeLogEvent.POLLING_STOPPED.log()
        listening = false
        awaitingFocus = false
        main.removeCallbacks(tick)
        main.removeCallbacks(focusTimeout)
        val context = app
        screenReceiver?.let { receiver -> runCatching { context?.unregisterReceiver(receiver) } }
        screenReceiver = null
        clipboardListener?.let { listener ->
            context?.getSystemService(ClipboardManager::class.java)?.removePrimaryClipChangedListener(listener)
        }
        clipboardListener = null
        val view = overlay
        val manager = windowManager
        // Removing a window during its focus dispatch crashes ViewRootImpl on some OEMs.
        if (view != null && manager != null) main.post { runCatching { manager.removeView(view) } }
        overlay = null
        params = null
        windowManager = null
        policy = null
        host = null
        onLost = null
        app = null
        AndroidClipboardReader.stopBackground(completion)
    }

    private fun listenDirectly(context: Context, opened: AndroidClipboardReader.Host) {
        clipboardListener = ClipboardManager.OnPrimaryClipChangedListener {
            if (!MainActivity.isForeground && host === opened) {
                AndroidClipboardReader.captureBackground(context, opened.id)
            }
        }.also { context.getSystemService(ClipboardManager::class.java)?.addPrimaryClipChangedListener(it) }
    }

    private fun addOverlay(context: Context) {
        val manager = context.getSystemService(WindowManager::class.java) ?: error("No window manager")
        // Read after Android returns from the focus dispatch, not inside it.
        val view = FocusProbe(context) { hasFocus ->
            if (hasFocus) main.post { if (awaitingFocus) read() }
        }
        val layout = WindowManager.LayoutParams(
            1,
            1,
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or
                WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL or
                WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS,
            PixelFormat.TRANSLUCENT,
        ).apply { gravity = Gravity.TOP or Gravity.START }
        manager.addView(view, layout)
        windowManager = manager
        overlay = view
        params = layout
    }

    private fun registerScreenReceiver(context: Context) {
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) {
                if (!listening) return
                if (intent.action == Intent.ACTION_SCREEN_OFF) {
                    RuntimeLogEvent.POLLING_PAUSED.log()
                    main.removeCallbacks(tick)
                    release()
                } else {
                    RuntimeLogEvent.POLLING_RESUMED.log()
                    schedule()
                }
            }
        }
        ContextCompat.registerReceiver(
            context,
            receiver,
            IntentFilter().apply {
                addAction(Intent.ACTION_SCREEN_OFF)
                addAction(Intent.ACTION_SCREEN_ON)
                addAction(Intent.ACTION_USER_PRESENT)
            },
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )
        screenReceiver = receiver
    }

    private fun schedule() {
        val interval = policy?.intervalMs ?: return
        main.removeCallbacks(tick)
        main.postDelayed(tick, interval)
    }

    private fun poll() {
        val context = app ?: return
        val policy = policy ?: return
        if (!listening || awaitingFocus) return
        if (!AndroidCaptureState.overlayGranted(context)) {
            onLost?.invoke()
            return
        }
        val step = policy.step(
            interactive = context.getSystemService(PowerManager::class.java)?.isInteractive != false,
            locked = context.getSystemService(KeyguardManager::class.java)?.isKeyguardLocked == true,
            appVisible = MainActivity.isForeground,
        )
        when (step) {
            PollingStep.PAUSE -> Unit
            PollingStep.READ -> focus()
        }
    }

    private fun focus() {
        val view = overlay ?: return
        val layout = params ?: return
        val manager = windowManager ?: return
        awaitingFocus = true
        try {
            // FLAG_ALT_FOCUSABLE_IM keeps the foreground app's keyboard in place.
            layout.flags = (layout.flags and WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE.inv()) or
                WindowManager.LayoutParams.FLAG_ALT_FOCUSABLE_IM
            manager.updateViewLayout(view, layout)
        } catch (_: RuntimeException) {
            release()
            return
        }
        if (view.hasWindowFocus()) read() else main.postDelayed(focusTimeout, ClipboardPollingPolicy.FOCUS_TIMEOUT_MS)
    }

    private fun read() {
        main.removeCallbacks(focusTimeout)
        val context = app
        val opened = host
        try {
            if (context != null && opened != null && !opened.closed.get()) {
                val clipboard = context.getSystemService(ClipboardManager::class.java)
                var timestamp: Long? = null
                AndroidClipboardReader.readCaptureMetadata(opened) {
                    timestamp = clipboard?.primaryClipDescription?.timestamp
                }
                // The full clip is read while this window still owns focus.
                if (policy?.changed(timestamp) == true) AndroidClipboardReader.captureBackground(context, opened.id)
            }
        } finally {
            release()
        }
    }

    private fun release() {
        main.removeCallbacks(focusTimeout)
        val wasAwaiting = awaitingFocus
        awaitingFocus = false
        val view = overlay
        val layout = params
        val manager = windowManager
        if (view != null && layout != null && manager != null &&
            layout.flags and WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE == 0
        ) {
            layout.flags = (layout.flags or WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE) and
                WindowManager.LayoutParams.FLAG_ALT_FOCUSABLE_IM.inv()
            runCatching { manager.updateViewLayout(view, layout) }
        }
        if (wasAwaiting && listening) schedule()
    }

    private class FocusProbe(context: Context, private val focusChanged: (Boolean) -> Unit) : View(context) {
        override fun onWindowFocusChanged(hasWindowFocus: Boolean) {
            super.onWindowFocusChanged(hasWindowFocus)
            focusChanged(hasWindowFocus)
        }
    }
}
