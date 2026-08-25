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
        val copy = state(this)
        if (intent == null || copy == null) {
            clearState(this)
            ClipCascadeCapture.disarm()
            if (copy != null) {
                CaptureNotifications.postLost(this, copy.lostTitle, copy.lostBody)
            }
            stopSelf(startId)
            return START_NOT_STICKY
        }

        // The FGS notification is the user's visible evidence that a reader
        // is alive. Never run an invisible service on Android 13+.
        if (!CaptureNotifications.canPost(this)) {
            clearState(this)
            ClipCascadeCapture.disarm()
            stopSelf(startId)
            return START_NOT_STICKY
        }
        CaptureNotifications.ensureChannels(this)
        startForeground(
            CaptureNotifications.ONGOING_ID,
            CaptureNotifications.ongoing(this, copy.ongoingText),
        )
        if (!ClipCascadeCapture.arm(this, {
                lost(this, copy)
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
        val copy = state(this)
        clearState(this)
        ClipCascadeCapture.disarm()
        if (copy != null) {
            CaptureNotifications.postLost(this, copy.lostTitle, copy.lostBody)
        }
        super.onDestroy()
    }

    companion object {
        private const val PREFS = "capture-service"
        private const val KEY_ENABLED = "enabled"
        private const val KEY_ONGOING_TEXT = "ongoingText"
        private const val KEY_LOST_TITLE = "lostTitle"
        private const val KEY_LOST_BODY = "lostBody"

        fun start(context: Context, copy: CaptureArmRequest): Boolean {
            if (copy.ongoingText.isBlank() || copy.lostTitle.isBlank() || copy.lostBody.isBlank()) {
                clearState(context)
                return false
            }
            if (!ClipCascadeCapture.arm(context, {
                    lost(context, copy)
                })) {
                clearState(context)
                return false
            }
            val persisted = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                .edit()
                .putBoolean(KEY_ENABLED, true)
                .putString(KEY_ONGOING_TEXT, copy.ongoingText)
                .putString(KEY_LOST_TITLE, copy.lostTitle)
                .putString(KEY_LOST_BODY, copy.lostBody)
                .commit()
            if (!persisted) {
                ClipCascadeCapture.disarm()
                clearState(context)
                return false
            }
            val intent = Intent(context, CaptureService::class.java)
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

        private fun lost(context: Context, copy: CaptureArmRequest) {
            clearState(context)
            ClipCascadeCapture.disarm()
            CaptureNotifications.postLost(context, copy.lostTitle, copy.lostBody)
            context.stopService(Intent(context, CaptureService::class.java))
        }

        fun isArmed(context: Context): Boolean = state(context) != null

        private fun clearState(context: Context) {
            context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().clear().apply()
        }

        private fun state(context: Context): CaptureArmRequest? {
            val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            if (!prefs.getBoolean(KEY_ENABLED, false)) return null
            val copy = CaptureArmRequest(
                ongoingText = prefs.getString(KEY_ONGOING_TEXT, null) ?: return null,
                lostTitle = prefs.getString(KEY_LOST_TITLE, null) ?: return null,
                lostBody = prefs.getString(KEY_LOST_BODY, null) ?: return null,
            )
            return copy.takeUnless {
                it.ongoingText.isBlank() || it.lostTitle.isBlank() || it.lostBody.isBlank()
            }
        }
    }
}
