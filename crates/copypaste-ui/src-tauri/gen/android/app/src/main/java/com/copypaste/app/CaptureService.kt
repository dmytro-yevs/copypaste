package com.copypaste.app

import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder

internal class CaptureStartCompletions {
    private val pending = mutableListOf<(Boolean) -> Unit>()

    @Synchronized
    fun add(completion: (Boolean) -> Unit) {
        pending += completion
    }

    @Synchronized
    fun remove(completion: (Boolean) -> Unit) {
        pending.remove(completion)
    }

    @Synchronized
    fun complete(started: Boolean) {
        val callbacks = pending.toList()
        pending.clear()
        callbacks.forEach { it(started) }
    }
}

/**
 * Keeps the process alive while rung 2 is armed.
 *
 * The Shizuku reader's callback reaches this process, so a reclaimed process
 * stops saving copies. This service does no work itself; it exists so the
 * callback endpoint and Rust store stay resident.
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
            !ClipCascadeCapture.isSetupComplete(this)
        ) {
            completeStart(false)
            ClipCascadeCapture.disarm()
            ClipQueue.markCaptureStateDirty()
            stopSelf(startId)
            return START_NOT_STICKY
        }

        CaptureNotifications.ensureChannels(this)
        startForeground(
            CaptureNotifications.ONGOING_ID,
            CaptureNotifications.ongoing(this, copy.ongoingText),
        )
        if (!ClipCascadeCapture.arm(
                this,
                onStarted = { started ->
                    if (started) {
                        completeStart(true)
                        ClipQueue.markCaptureStateDirty()
                    } else {
                        lost(this, copy)
                    }
                },
                onLost = { lost(this, copy) },
            )) {
            lost(this, copy)
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        completeStart(false)
        ClipCascadeCapture.disarm()
        ClipQueue.markCaptureStateDirty()
        super.onDestroy()
    }

    companion object {
        private const val PREFS = "capture-service"
        private const val KEY_WANTED = "wanted"
        private const val KEY_ENABLED = "enabled"
        private const val KEY_ONGOING_TEXT = "ongoingText"
        private const val KEY_LOST_TITLE = "lostTitle"
        private const val KEY_LOST_BODY = "lostBody"
        private const val KEY_RECOVERY_REQUIRED = "recoveryRequired"

        fun rememberWanted(context: Context, wanted: Boolean) {
            writeWanted(context, wanted)
        }

        fun rememberArm(context: Context, copy: CaptureArmRequest): Boolean {
            if (copy.ongoingText.isBlank() || copy.lostTitle.isBlank() || copy.lostBody.isBlank()) {
                return false
            }
            writeWanted(context, true)
            setRecoveryRequired(context, false)
            return true
        }

        fun start(
            context: Context,
            copy: CaptureArmRequest,
            completion: ((Boolean) -> Unit)? = null,
        ): Boolean {
            if (copy.ongoingText.isBlank() || copy.lostTitle.isBlank() || copy.lostBody.isBlank()) {
                return false
            }
            writeWanted(context, true)
            setRecoveryRequired(context, false)
            if (!ClipCascadeCapture.isSetupComplete(context)) {
                return false
            }
            if (!persistCopy(context, copy)) {
                return false
            }
            completion?.let(::rememberStartCompletion)
            if (startService(context)) return true
            completion?.let(::forgetStartCompletion)
            return false
        }

        fun restoreIfArmed(context: Context): Boolean {
            if (!userWantsCapture(context)) return false
            if (recoveryRequired(context)) return false
            if (notificationCopy(context) == null) return false
            if (ClipCascadeCapture.isListening()) return true
            if (!ClipCascadeCapture.isSetupComplete(context)) return false
            if (!CaptureNotifications.canPost(context)) return false
            return startService(context)
        }

        fun stop(context: Context) {
            completeStart(false)
            writeWanted(context, false)
            clearCopy(context)
            setRecoveryRequired(context, false)
            ClipCascadeCapture.disarm()
            ClipQueue.markCaptureStateDirty()
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

        fun recoveryRequired(context: Context): Boolean =
            context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                .getBoolean(KEY_RECOVERY_REQUIRED, false)

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
            completeStart(false)
            ClipCascadeCapture.disarm()
            setRecoveryRequired(context, true)
            ClipQueue.markCaptureStateDirty()
            CaptureNotifications.postLost(context, copy.lostTitle, copy.lostBody)
            context.stopService(Intent(context, CaptureService::class.java))
        }

        @Synchronized
        private fun rememberStartCompletion(completion: (Boolean) -> Unit) {
            startCompletions.add(completion)
        }

        private fun forgetStartCompletion(completion: (Boolean) -> Unit) {
            startCompletions.remove(completion)
        }

        private fun completeStart(started: Boolean) {
            startCompletions.complete(started)
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

        private fun setRecoveryRequired(context: Context, required: Boolean) {
            context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                .edit().putBoolean(KEY_RECOVERY_REQUIRED, required).commit()
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

        private val startCompletions = CaptureStartCompletions()
    }
}
