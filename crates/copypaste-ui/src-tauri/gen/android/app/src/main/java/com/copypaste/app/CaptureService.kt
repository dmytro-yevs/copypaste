package com.copypaste.app

import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder

/**
 * Keeps the process alive while rung 2 is armed.
 *
 * The logcat reader is a callback into this process, so a reclaimed process
 * stops saving copies. This service does no work itself; it exists so the
 * reader and the Rust store stay resident.
 *
 * The on/off choice is persisted independently. [restoreIfArmed] re-arms from
 * those prefs only when the runtime grants that make the reader work are still
 * present. [START_STICKY] would resurrect a reader-less service after OEM
 * process death; OEM kills fail closed.
 */
class CaptureService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val copy = notificationCopy(this)
        if (!userWantsCapture(this) ||
            copy == null ||
            !CaptureNotifications.canPost(this) ||
            !ClipCascadeCapture.hasRuntimePermissions(this)
        ) {
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
            CaptureNotifications.postLost(this, copy.lostTitle, copy.lostBody)
            stopSelf(startId)
            return START_NOT_STICKY
        }
        return START_NOT_STICKY
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
            writeWanted(context, true)
            return true
        }

        fun start(context: Context, copy: CaptureArmRequest): Boolean {
            if (copy.ongoingText.isBlank() || copy.lostTitle.isBlank() || copy.lostBody.isBlank()) {
                return false
            }
            writeWanted(context, true)
            if (!ClipCascadeCapture.arm(context, {
                    lost(context, copy)
                })) {
                return false
            }
            if (!persistCopy(context, copy)) {
                ClipCascadeCapture.disarm()
                return false
            }
            return startService(context)
        }

        fun restoreIfArmed(context: Context): Boolean {
            if (!userWantsCapture(context)) return false
            if (notificationCopy(context) == null) return false
            if (ClipCascadeCapture.isListening()) return true
            if (!ClipCascadeCapture.hasRuntimePermissions(context)) return false
            if (!CaptureNotifications.canPost(context)) return false
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

        private fun persistCopy(context: Context, copy: CaptureArmRequest): Boolean =
            context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                .edit()
                .putString(KEY_ONGOING_TEXT, copy.ongoingText)
                .putString(KEY_LOST_TITLE, copy.lostTitle)
                .putString(KEY_LOST_BODY, copy.lostBody)
                .commit()

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
