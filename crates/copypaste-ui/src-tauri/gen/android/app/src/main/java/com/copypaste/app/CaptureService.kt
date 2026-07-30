package com.copypaste.app

import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder

/**
 * Keeps the process alive while rung 2 is armed.
 *
 * The clip listener is a callback into *this* process, so if the process is
 * reclaimed the listener goes with it and copies stop being saved. A foreground
 * service is the only thing Android offers that says "keep me". It does no work
 * itself; it exists so that [ShizukuClipboard]'s listener and the Rust store
 * are both still there when someone copies in another app.
 *
 * The ongoing notification is not an apology for the service — it is the
 * android doc's §5 rule 1 outside the app: capture state is visible wherever
 * the user is, not only where history is.
 */
class CaptureService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        CaptureNotifications.ensureChannels(this)
        val text = intent?.getStringExtra(EXTRA_TEXT) ?: "Capturing from every app."
        startForeground(CaptureNotifications.ONGOING_ID, CaptureNotifications.ongoing(this, text))
        // Restarted after a kill, because the user's expectation is that this
        // keeps running. It re-arms through the plugin's next probe rather than
        // arming here: whether the grant survived is not this class's decision.
        return START_STICKY
    }

    companion object {
        private const val EXTRA_TEXT = "text"

        fun start(context: Context, text: String) {
            val intent = Intent(context, CaptureService::class.java).putExtra(EXTRA_TEXT, text)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, CaptureService::class.java))
        }
    }
}
