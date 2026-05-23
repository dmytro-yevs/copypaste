package com.copypaste.android

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log

/**
 * Receives Pause/Resume actions fired from the foreground-service notification
 * (see [ClipboardService.buildNotification]). Flips the persisted
 * [Settings.captureEnabled] flag and re-issues the notification so the
 * Pause/Resume button label updates immediately.
 *
 * Declared `android:exported="false"` in the manifest — only our own
 * notification PendingIntents may trigger it.
 */
class CaptureControlReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val settings = Settings(context)
        when (intent.action) {
            ACTION_PAUSE -> {
                settings.captureEnabled = false
                Log.d(TAG, "Capture paused via notification action")
            }
            ACTION_RESUME -> {
                settings.captureEnabled = true
                Log.d(TAG, "Capture resumed via notification action")
            }
            else -> {
                Log.w(TAG, "Unknown action: ${intent.action}")
                return
            }
        }
        ClipboardService.refreshNotification(context)
    }

    companion object {
        private const val TAG = "CaptureControlReceiver"
        const val ACTION_PAUSE = "com.copypaste.android.action.PAUSE_CAPTURE"
        const val ACTION_RESUME = "com.copypaste.android.action.RESUME_CAPTURE"
    }
}
