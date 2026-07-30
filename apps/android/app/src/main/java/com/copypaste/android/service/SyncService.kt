package com.copypaste.android.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import androidx.core.content.getSystemService
import com.copypaste.android.MainActivity
import com.copypaste.android.R

/**
 * The foreground service.
 *
 * # What it is not
 *
 * **It does not poll the clipboard, and it must never be changed to.** Since
 * Android 10 (API 29) `ClipboardManager.getPrimaryClip()` returns `null` to any
 * app that is neither the focused app nor the active input method. That is not
 * a permission that can be requested — there is no permission — and it is not
 * a restriction a foreground service lifts. A background clipboard poller on a
 * modern Android device reads `null` forever while burning the user's battery,
 * and looks like a working feature to everyone except the user.
 *
 * `OnPrimaryClipChangedListener` still fires in the background on many devices,
 * which makes this trap easy to fall into: the callback arrives, and the
 * content behind it does not. Registering it here would produce a service that
 * appears to work in a log and stores nothing.
 *
 * So this app captures from the two routes the platform actually supports:
 * while it has focus (`MainActivity`), and when the user sends text to it
 * (share sheet, `ACTION_PROCESS_TEXT`). `apps/android/README.md` says the same
 * thing at more length, because it is the single most surprising thing about
 * this app.
 *
 * # What it is for
 *
 * Keeping the process alive so peer operations survive the user switching away
 * mid-sync, and giving the user a visible, cancellable handle on that. Declared
 * `dataSync` in the manifest, with the matching
 * `FOREGROUND_SERVICE_DATA_SYNC` permission that API 34 requires.
 *
 * On API 35+ a `dataSync` service has a rolling budget of roughly six hours per
 * 24, after which [onTimeout] is called and the service must stop itself
 * promptly or be killed. That is handled below rather than ignored: the work
 * this service protects is measured in seconds, so hitting the timeout means
 * the service was left running with nothing to do, and stopping is correct.
 */
class SyncService : Service() {

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        createChannel()
        val notification = buildNotification()

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            // API 34 requires the type at start time as well as in the
            // manifest, and refuses the start if they disagree.
            startForeground(
                NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC,
            )
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }

        // START_NOT_STICKY: if the system kills us there is nothing to resume.
        // Restarting would put a notification back in front of the user for
        // work that no longer exists.
        return START_NOT_STICKY
    }

    /**
     * API 35+: the `dataSync` budget ran out.
     *
     * Must stop quickly — the system kills the process otherwise, and an ANR
     * attributed to this service is worse than a stopped one.
     */
    override fun onTimeout(startId: Int, fgsType: Int) {
        stopSelf()
    }

    private fun createChannel() {
        val channel = NotificationChannel(
            CHANNEL_ID,
            getString(R.string.service_channel_name),
            // LOW: no sound, no heads-up. The notification is a handle and a
            // disclosure, not an interruption.
            NotificationManager.IMPORTANCE_LOW,
        ).apply {
            description = getString(R.string.service_channel_description)
            setShowBadge(false)
        }
        getSystemService<NotificationManager>()?.createNotificationChannel(channel)
    }

    private fun buildNotification(): Notification {
        val open = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle(getString(R.string.service_notification_title))
            // Says what the service is doing, and does not imply it is watching
            // the clipboard. A notification that overstates what an app can see
            // is its own kind of dishonesty.
            .setContentText(getString(R.string.service_notification_body))
            .setSmallIcon(R.drawable.ic_notification)
            .setContentIntent(open)
            .setOngoing(true)
            .setSilent(true)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            // Never a clipping in the notification: the lock screen is a
            // surface this app does not control.
            .setVisibility(NotificationCompat.VISIBILITY_SECRET)
            .build()
    }

    companion object {
        private const val CHANNEL_ID = "copypaste.sync"
        private const val NOTIFICATION_ID = 1

        /**
         * Start the service.
         *
         * The caller must already hold `POST_NOTIFICATIONS` on API 33+. Without
         * it the service still runs but its notification is suppressed, which
         * is a background service the user can neither see nor stop — so the
         * app asks first and simply does not start it if the answer is no.
         */
        fun start(context: Context) {
            ContextCompat.startForegroundService(
                context,
                Intent(context, SyncService::class.java),
            )
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, SyncService::class.java))
        }
    }
}
