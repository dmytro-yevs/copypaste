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
 * The user's on/off choice is persisted independently of this object and of
 * whether the reader is listening right now. Process death, a dismissed
 * permission prompt, and a failed arm must not write that choice off —
 * [START_STICKY] restarts from the same prefs, and a null intent is a restart,
 * not a disarm.
 */
class CaptureService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (!userWantsCapture(this)) {
            ClipCascadeCapture.disarm()
            stopSelf(startId)
            return START_NOT_STICKY
        }

        val copy = notificationCopy(this)
        if (copy == null) {
            ClipCascadeCapture.disarm()
            stopSelf(startId)
            return START_NOT_STICKY
        }

        if (!CaptureNotifications.canPost(this)) {
            ClipCascadeCapture.disarm()
            CaptureNotifications.postLost(this, copy.lostTitle, copy.lostBody)
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
            CaptureNotifications.postLost(this, copy.lostTitle, copy.lostBody)
            stopSelf(startId)
            return START_NOT_STICKY
        }
        return START_STICKY
    }

    override fun onDestroy() {
        ClipCascadeCapture.disarm()
        super.onDestroy()
    }

    companion object {
        private const val PREFS = "capture-service"
        private const val KEY_WANTED = "wanted"
        private const val KEY_ENABLED = "enabled"
        private const val KEY_ONGOING_TEXT = "ongoingText"
        private const val KEY_LOST_TITLE = "lostTitle"
        private const val KEY_LOST_BODY = "lostBody"

        fun rememberWanted(context: Context, wanted: Boolean) {
            writeWanted(context, wanted)
        }

        fun rememberArm(context: Context, copy: CaptureArmRequest): Boolean {
            if (copy.ongoingText.isBlank() || copy.lostTitle.isBlank() || copy.lostBody.isBlank()) {
                return false
            }
            persistWantedAndCopy(context, wanted = true, copy)
            return true
        }

        fun start(context: Context, copy: CaptureArmRequest): Boolean {
            if (copy.ongoingText.isBlank() || copy.lostTitle.isBlank() || copy.lostBody.isBlank()) {
                return false
            }
            persistWantedAndCopy(context, wanted = true, copy)
            ClipCascadeCapture.arm(context, {
                lost(context, copy)
            })
            return startService(context)
        }

        fun restoreIfArmed(context: Context): Boolean {
            if (!userWantsCapture(context)) return false
            if (notificationCopy(context) == null) return false
            if (ClipCascadeCapture.isListening()) return true
            return startService(context)
        }

        fun stop(context: Context) {
            writeWanted(context, false)
            clearCopy(context)
            ClipCascadeCapture.disarm()
            context.stopService(Intent(context, CaptureService::class.java))
        }

        fun userWantsCapture(context: Context): Boolean {
            val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            if (!prefs.contains(KEY_WANTED)) {
                prefs.edit().putBoolean(KEY_WANTED, true).commit()
                return true
            }
            return prefs.getBoolean(KEY_WANTED, true)
        }

        fun isArmed(context: Context): Boolean =
            userWantsCapture(context) && notificationCopy(context) != null

        private fun startService(context: Context): Boolean {
            val intent = Intent(context, CaptureService::class.java)
            return try {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    context.startForegroundService(intent)
                } else {
                    context.startService(intent)
                }
                true
            } catch (_: Exception) {
                false
            }
        }

        private fun lost(context: Context, copy: CaptureArmRequest) {
            ClipCascadeCapture.disarm()
            CaptureNotifications.postLost(context, copy.lostTitle, copy.lostBody)
            context.stopService(Intent(context, CaptureService::class.java))
        }

        private fun persistWantedAndCopy(context: Context, wanted: Boolean, copy: CaptureArmRequest) {
            context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                .edit()
                .putBoolean(KEY_WANTED, wanted)
                .putBoolean(KEY_ENABLED, wanted)
                .putString(KEY_ONGOING_TEXT, copy.ongoingText)
                .putString(KEY_LOST_TITLE, copy.lostTitle)
                .putString(KEY_LOST_BODY, copy.lostBody)
                .commit()
        }

        private fun writeWanted(context: Context, wanted: Boolean) {
            context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                .edit()
                .putBoolean(KEY_WANTED, wanted)
                .putBoolean(KEY_ENABLED, wanted)
                .commit()
        }

        private fun clearCopy(context: Context) {
            context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                .edit()
                .remove(KEY_ONGOING_TEXT)
                .remove(KEY_LOST_TITLE)
                .remove(KEY_LOST_BODY)
                .apply()
        }

        private fun notificationCopy(context: Context): CaptureArmRequest? {
            val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
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
