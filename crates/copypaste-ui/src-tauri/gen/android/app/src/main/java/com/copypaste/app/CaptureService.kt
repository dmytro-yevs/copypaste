package com.copypaste.app

import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder

/**
 * Keeps the process alive while rung 2 is armed.
 *
 * The logcat reader is a callback path into *this* process, so if the process
 * is reclaimed the reader goes with it and copies stop being saved. A
 * foreground service is the only thing Android offers that says "keep me".
 * It does no work itself; it exists so the reader and the Rust store are both
 * still there when someone copies in another app.
 *
 * CopyPaste's ClipCascade path is app-owned after setup: Shizuku only applies
 * the one-shot grants. This service keeps the logcat reader alive and visible.
 *
 * The ongoing notification is not an apology for the service — it is the
 * android doc's §5 rule 1 outside the app: capture state is visible wherever
 * the user is, not only where history is.
 */
class CaptureService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val state = state(this)
        if (intent == null || !state.enabled) {
            clearState(this)
            ClipCascadeCapture.disarm()
            if (state.enabled) {
                CaptureNotifications.postLost(this, state.lostTitle, state.lostBody)
            }
            stopSelf(startId)
            return START_NOT_STICKY
        }

        // The FGS notification is the user's visible evidence that a listener
        // is alive. Never run an invisible service on Android 13+.
        if (!CaptureNotifications.canPost(this)) {
            clearState(this)
            ClipCascadeCapture.disarm()
            stopSelf(startId)
            return START_NOT_STICKY
        }
        CaptureNotifications.ensureChannels(this)
        val text = intent.getStringExtra(EXTRA_TEXT) ?: state.text
        startForeground(CaptureNotifications.ONGOING_ID, CaptureNotifications.ongoing(this, text))
        if (!ClipCascadeCapture.arm(this, {
                lost(this, state.lostTitle, state.lostBody)
            })) {
            clearState(this)
            stopSelf(startId)
            return START_NOT_STICKY
        }
        // OEM process death must not resurrect this service with a null intent.
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        // The callback belongs to this process, not the service object. An
        // unexpected teardown cannot leave a persisted green state behind.
        val state = state(this)
        clearState(this)
        ClipCascadeCapture.disarm()
        if (state.enabled) {
            CaptureNotifications.postLost(this, state.lostTitle, state.lostBody)
        }
        super.onDestroy()
    }

    companion object {
        private const val EXTRA_TEXT = "text"
        private const val EXTRA_LOST_TITLE = "lostTitle"
        private const val EXTRA_LOST_BODY = "lostBody"
        private const val PREFS = "capture-service"
        private const val KEY_ENABLED = "enabled"
        private const val KEY_TEXT = "text"
        private const val KEY_LOST_TITLE = "lostTitle"
        private const val KEY_LOST_BODY = "lostBody"

        fun start(context: Context, text: String, lostTitle: String, lostBody: String): Boolean {
            if (!ClipCascadeCapture.arm(context, {
                    lost(context, lostTitle, lostBody)
                })) {
                clearState(context)
                return false
            }
            context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                .edit()
                .putBoolean(KEY_ENABLED, true)
                .putString(KEY_TEXT, text)
                .putString(KEY_LOST_TITLE, lostTitle)
                .putString(KEY_LOST_BODY, lostBody)
                .apply()
            val intent = Intent(context, CaptureService::class.java)
                .putExtra(EXTRA_TEXT, text)
                .putExtra(EXTRA_LOST_TITLE, lostTitle)
                .putExtra(EXTRA_LOST_BODY, lostBody)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
            return true
        }

        fun stop(context: Context) {
            clearState(context)
            ClipCascadeCapture.disarm()
            context.stopService(Intent(context, CaptureService::class.java))
        }

        fun lost(context: Context, title: String, body: String) {
            clearState(context)
            ClipCascadeCapture.disarm()
            CaptureNotifications.postLost(context, title, body)
            context.stopService(Intent(context, CaptureService::class.java))
        }

        fun isArmed(context: Context): Boolean = state(context).enabled

        private fun clearState(context: Context) {
            context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().clear().apply()
        }

        private fun state(context: Context): CaptureState {
            val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            return CaptureState(
                enabled = prefs.getBoolean(KEY_ENABLED, false),
                text = prefs.getString(KEY_TEXT, "Capturing from every app.")
                    ?: "Capturing from every app.",
                lostTitle = prefs.getString(KEY_LOST_TITLE, "Background capture stopped.")
                    ?: "Background capture stopped.",
                lostBody = prefs.getString(KEY_LOST_BODY, "") ?: "",
            )
        }
    }

    private data class CaptureState(
        val enabled: Boolean,
        val text: String,
        val lostTitle: String,
        val lostBody: String,
    )
}
