package com.copypaste.android

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.media.AudioManager
import android.os.Build
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import java.util.Calendar

/**
 * CopyPaste-vp63.32: notification channels, foreground-service notification
 * builder, per-copy event toast, incoming-pair alert, and the "today captured"
 * counter surface — extracted VERBATIM from [ClipboardService]'s companion
 * object. [ClipboardService] keeps forwarding stubs (same names/signatures)
 * so external callers ([CaptureControlReceiver], [ServiceRestartWorker],
 * [ClipboardRepository], [DevicesActivity], [Settings] doc links, and the
 * per-copy/capture pipeline) are unaffected.
 *
 * Behaviour-preserving: no logic changed, only relocated. Log tag kept as
 * "ClipboardService" for log continuity (existing logcat filters/greps still
 * match).
 */
object ServiceNotifications {
    private const val TAG = "ClipboardService"

    const val NOTIFICATION_ID = 1001
    const val CHANNEL_ID = "copypaste_service"

    // ── Deliverable 1: incoming-pair notification ─────────────────────────

    /** HIGH-importance channel for incoming SAS pairing requests. */
    const val CHANNEL_PAIR_REQUEST = "copypaste_pair_request"

    /** Stable notification id for the incoming-pair prompt (one at a time). */
    const val NOTIF_ID_PAIR_REQUEST = 1004

    /**
     * Notification channel for per-copy event toasts (A-SET-6 parity).
     * IMPORTANCE_MIN = no sound, no heads-up, no status-bar icon — just a
     * silent badge in the shade so the user can see "item captured" without
     * being disturbed. Auto-cancelled after 2 seconds.
     */
    const val CHANNEL_COPY_EVENT = "copypaste_copy_event"

    /** Stable notification id for the per-copy event notification. */
    private const val NOTIF_ID_COPY_EVENT = 1003

    /**
     * CopyPaste-myh8.9: stable notification id for the sensitive-upload-skipped
     * alert (see [postSensitiveSkipNotification]). Distinct from
     * [NOTIF_ID_COPY_EVENT] so the two never clobber each other when both fire
     * close together.
     */
    private const val NOTIF_ID_SENSITIVE_SKIP = 1005

    /**
     * Debounce guard: timestamp (System.currentTimeMillis) of the last copy
     * notification. If another capture arrives within [COPY_NOTIF_DEBOUNCE_MS],
     * the notification is refreshed in-place (same id) rather than posting a
     * new one, preventing rapid bursts from stacking.
     */
    @Volatile
    private var lastCopyNotifMs = 0L
    private const val COPY_NOTIF_DEBOUNCE_MS = 500L

    private const val PREFS_NAME = "copypaste_notif"
    private const val KEY_DAY_BUCKET = "day_bucket"
    private const val KEY_TODAY_COUNT = "today_count"

    /**
     * Post (or refresh) the per-copy event notification.
     *
     * Debounced: if the previous notification was posted within
     * [COPY_NOTIF_DEBOUNCE_MS], this call updates it in-place (same id)
     * rather than emitting a new one — rapid-paste bursts produce a single
     * updating notification rather than a stack.
     *
     * Requires POST_NOTIFICATIONS permission on API 33+; on older APIs the
     * permission is implicit. [NotificationManagerCompat.notify] is a no-op
     * when the permission has not been granted, so no guard is needed here.
     */
    fun postCopyNotification(context: Context) {
        val now = System.currentTimeMillis()
        // Atomic CAS-style update: read, decide, write under no lock — worst
        // case two threads both post; that is fine (same stable id, idempotent).
        lastCopyNotifMs = now
        ensureChannel(context)
        val notification = NotificationCompat.Builder(context, CHANNEL_COPY_EVENT)
            .setSmallIcon(R.drawable.ic_stat_notify)
            .setContentTitle(context.getString(R.string.notif_copy_event_title))
            .setContentText(context.getString(R.string.notif_copy_event_content))
            .setPriority(NotificationCompat.PRIORITY_MIN)
            .setCategory(NotificationCompat.CATEGORY_EVENT)
            .setAutoCancel(true)
            .setTimeoutAfter(2_000L)
            .setOnlyAlertOnce(true)
            .build()
        // POST_NOTIFICATIONS is revocable on API 33+. Guard with explicit
        // checkSelfPermission so lint is satisfied; also catch SecurityException
        // as a belt-and-suspenders fallback (notification miss is non-fatal).
        val canNotify = if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
            androidx.core.content.ContextCompat.checkSelfPermission(
                context, android.Manifest.permission.POST_NOTIFICATIONS,
            ) == android.content.pm.PackageManager.PERMISSION_GRANTED
        } else {
            true
        }
        if (canNotify) {
            try {
                NotificationManagerCompat.from(context).notify(NOTIF_ID_COPY_EVENT, notification)
            } catch (se: SecurityException) {
                Log.w(TAG, "postCopyNotification: permission revoked mid-flight (non-fatal): ${se.message}")
            }
        }
    }

    /**
     * CopyPaste-myh8.9: post (or refresh) the sensitive-upload-skipped notification.
     * Gated by [Settings.notifyOnSensitiveSkip] — the caller ([ClipboardCapturePipeline.captureClip])
     * checks the setting before invoking this.
     *
     * Reuses [CHANNEL_COPY_EVENT] (same silent, IMPORTANCE_MIN badge-only channel as
     * [postCopyNotification]) rather than adding a new channel — this is a sibling
     * per-capture event notification, not a distinct notification category.
     *
     * SECURITY: the notification text is a GENERIC localized string (R.string.notif_sensitive_skip_content)
     * — it must NEVER contain the clip's plaintext or any derived preview of it.
     */
    fun postSensitiveSkipNotification(context: Context) {
        ensureChannel(context)
        val notification = NotificationCompat.Builder(context, CHANNEL_COPY_EVENT)
            .setSmallIcon(R.drawable.ic_stat_notify)
            .setContentTitle(context.getString(R.string.notif_sensitive_skip_title))
            .setContentText(context.getString(R.string.notif_sensitive_skip_content))
            .setPriority(NotificationCompat.PRIORITY_MIN)
            .setCategory(NotificationCompat.CATEGORY_EVENT)
            .setAutoCancel(true)
            .setTimeoutAfter(4_000L)
            .setOnlyAlertOnce(true)
            .build()
        val canNotify = if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
            androidx.core.content.ContextCompat.checkSelfPermission(
                context, android.Manifest.permission.POST_NOTIFICATIONS,
            ) == android.content.pm.PackageManager.PERMISSION_GRANTED
        } else {
            true
        }
        if (canNotify) {
            try {
                NotificationManagerCompat.from(context).notify(NOTIF_ID_SENSITIVE_SKIP, notification)
            } catch (se: SecurityException) {
                Log.w(TAG, "postSensitiveSkipNotification: permission revoked mid-flight (non-fatal): ${se.message}")
            }
        }
    }

    /**
     * Play a subtle UI click sound to acknowledge a clipboard capture.
     *
     * Uses [AudioManager.playSoundEffect] with [SoundEffectConstants.CLICK],
     * which respects the system "touch sounds" volume and is available on all
     * API levels. The call is intentionally non-blocking and fire-and-forget.
     * Errors are swallowed — a missing sound must never break capture.
     */
    fun playCopySound(context: Context) {
        try {
            val am = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
            // AudioManager.playSoundEffect requires an AudioManager.FX_* constant;
            // FX_KEY_CLICK is the closest equivalent to a UI tap feedback sound.
            am.playSoundEffect(AudioManager.FX_KEY_CLICK, -1f)
        } catch (e: Exception) {
            Log.d(TAG, "playCopySound failed (non-fatal): ${e.message}")
        }
    }

    /**
     * Ensure all notification channels exist. Idempotent — calling twice is a
     * no-op on the framework side (createNotificationChannel is idempotent).
     *
     * [CHANNEL_ID]: IMPORTANCE_LOW = silent (no sound, no heads-up).
     *   setShowBadge(false) keeps the launcher icon clean.
     *
     * [CHANNEL_COPY_EVENT]: IMPORTANCE_MIN = silent badge only, no heads-up.
     */
    fun ensureChannel(context: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val nm = context.getSystemService(NotificationManager::class.java) ?: return

        if (nm.getNotificationChannel(CHANNEL_ID) == null) {
            nm.createNotificationChannel(
                NotificationChannel(
                    CHANNEL_ID,
                    context.getString(R.string.notif_channel_service_name),
                    NotificationManager.IMPORTANCE_LOW
                ).apply {
                    description = context.getString(R.string.notif_channel_service_description)
                    setShowBadge(false)
                    enableVibration(false)
                    setSound(null, null)
                }
            )
        }

        if (nm.getNotificationChannel(CHANNEL_COPY_EVENT) == null) {
            nm.createNotificationChannel(
                NotificationChannel(
                    CHANNEL_COPY_EVENT,
                    context.getString(R.string.notif_channel_copy_event_name),
                    NotificationManager.IMPORTANCE_MIN
                ).apply {
                    description = context.getString(R.string.notif_channel_copy_event_description)
                    setShowBadge(false)
                    enableVibration(false)
                    setSound(null, null)
                }
            )
        }

        // Deliverable 1: HIGH-importance channel for incoming pairing requests.
        if (nm.getNotificationChannel(CHANNEL_PAIR_REQUEST) == null) {
            nm.createNotificationChannel(
                NotificationChannel(
                    CHANNEL_PAIR_REQUEST,
                    context.getString(R.string.notif_channel_pair_request_name),
                    NotificationManager.IMPORTANCE_HIGH
                ).apply {
                    description = context.getString(R.string.notif_channel_pair_request_description)
                    setShowBadge(true)
                }
            )
        }
    }

    /**
     * Deliverable 1 — post (or refresh) a HIGH-priority notification alerting
     * the user that a peer wants to pair with this device. The tap intent opens
     * [DevicesActivity] where the SAS confirmation modal auto-opens.
     *
     * [peerName] is the discovered peer's device name (may be blank — falls back
     * to the generic string). Idempotent (same stable [NOTIF_ID_PAIR_REQUEST]).
     */
    fun postIncomingPairNotification(context: Context, peerName: String) {
        ensureChannel(context)
        val nm = context.getSystemService(NotificationManager::class.java) ?: return

        val piFlags = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        val devicesIntent = Intent(context, DevicesActivity::class.java).apply {
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
            // Signal DevicesActivity to auto-open the SAS modal on resume.
            putExtra(DevicesActivity.EXTRA_AUTO_OPEN_SAS, true)
        }
        val devicesPi = PendingIntent.getActivity(context, 20, devicesIntent, piFlags)

        val content = if (peerName.isNotBlank()) {
            context.getString(R.string.notif_pair_request_content, peerName)
        } else {
            context.getString(R.string.notif_pair_request_content_unknown)
        }

        val notification = NotificationCompat.Builder(context, CHANNEL_PAIR_REQUEST)
            .setSmallIcon(R.drawable.ic_stat_notify)
            .setContentTitle(context.getString(R.string.notif_pair_request_title))
            .setContentText(content)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setCategory(NotificationCompat.CATEGORY_EVENT)
            .setAutoCancel(true)
            .setContentIntent(devicesPi)
            .addAction(0, context.getString(R.string.notif_pair_action_confirm), devicesPi)
            .build()

        try {
            nm.notify(NOTIF_ID_PAIR_REQUEST, notification)
        } catch (e: SecurityException) {
            Log.w(TAG, "postIncomingPairNotification: POST_NOTIFICATIONS blocked: ${e.message}")
        }
    }

    /**
     * Re-issue the foreground notification using current [Settings] state.
     * Called by [CaptureControlReceiver] after toggling pause/resume, and
     * by the capture pipeline after each successful capture so the count updates.
     */
    fun refreshNotification(context: Context) {
        val nm = context.getSystemService(NotificationManager::class.java) ?: return
        ensureChannel(context)
        nm.notify(NOTIFICATION_ID, buildNotification(context))
    }

    /**
     * Guards [bumpTodayCounter]'s read-modify-write against concurrent callers
     * (ClipboardService + LogcatCaptureService both call captureClip/
     * captureImageClip on the IO dispatcher and can race on the same prefs file).
     */
    private val counterLock = Any()

    /**
     * Bump today's capture counter. Rolls over at local midnight (uses
     * day-of-year as the bucket key so the rollover is visible the
     * morning after).
     *
     * Guarded by [counterLock] to prevent a lost-update between the read of
     * KEY_TODAY_COUNT and the write of KEY_TODAY_COUNT + 1.
     */
    fun bumpTodayCounter(context: Context) {
        val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
        synchronized(counterLock) {
            val today = todayBucket()
            val storedBucket = prefs.getInt(KEY_DAY_BUCKET, -1)
            val current = if (storedBucket == today) prefs.getInt(KEY_TODAY_COUNT, 0) else 0
            prefs.edit()
                .putInt(KEY_DAY_BUCKET, today)
                .putInt(KEY_TODAY_COUNT, current + 1)
                .apply()
        }
    }

    /**
     * Reconcile the "captured today" counter after the user removes clips.
     * Decrements by [count] (floored at 0) and re-issues the notification so
     * the shown number reflects the store after a delete/clear. The counter
     * is otherwise monotonic-on-capture, so without this a deletion left the
     * notification reporting a stale, too-high total. Safe to call from any
     * thread — SharedPreferences and NotificationManager are both
     * thread-safe.
     */
    fun onItemsDeleted(context: Context, count: Int) {
        if (count <= 0) return
        val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
        val today = todayBucket()
        val storedBucket = prefs.getInt(KEY_DAY_BUCKET, -1)
        // Only adjust when the stored bucket is today's — a delete of an
        // older clip must not resurrect/zero a fresh day's bucket.
        if (storedBucket != today) {
            refreshNotification(context)
            return
        }
        val current = prefs.getInt(KEY_TODAY_COUNT, 0)
        val next = (current - count).coerceAtLeast(0)
        prefs.edit()
            .putInt(KEY_DAY_BUCKET, today)
            .putInt(KEY_TODAY_COUNT, next)
            .apply()
        refreshNotification(context)
    }

    private fun readTodayCount(context: Context): Int {
        val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
        val today = todayBucket()
        val storedBucket = prefs.getInt(KEY_DAY_BUCKET, -1)
        return if (storedBucket == today) prefs.getInt(KEY_TODAY_COUNT, 0) else 0
    }

    private fun todayBucket(): Int {
        val cal = Calendar.getInstance()
        // YYYY * 1000 + DOY — unique per local day, monotonically increasing
        // across year boundaries.
        return cal.get(Calendar.YEAR) * 1000 + cal.get(Calendar.DAY_OF_YEAR)
    }

    /**
     * Build the foreground-service notification. Visible state:
     *  - Title: "Active" or "Paused" depending on [Settings.captureEnabled]
     *  - Body: "<N> items captured today" / "Capture paused..."
     *  - Actions: Pause/Resume (toggle), Open (launch MainActivity)
     */
    fun buildNotification(context: Context): Notification {
        ensureChannel(context)
        val settings = Settings(context)
        val paused = !settings.captureEnabled
        val count = readTodayCount(context)

        val title = context.getString(
            if (paused) R.string.notif_title_paused else R.string.notif_title_active
        )
        val content = if (paused) {
            context.getString(R.string.notif_content_paused)
        } else {
            context.resources.getQuantityString(R.plurals.notif_content_today, count, count)
        }

        // Pending-intent flag set: IMMUTABLE is required on API 31+, allowed
        // on older releases (NotificationCompat handles back-compat).
        val piFlags = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE

        val openIntent = Intent(context, MainActivity::class.java).apply {
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
        }
        val openPi = PendingIntent.getActivity(context, 0, openIntent, piFlags)

        val toggleAction = if (paused) {
            CaptureControlReceiver.ACTION_RESUME to R.string.notif_action_resume
        } else {
            CaptureControlReceiver.ACTION_PAUSE to R.string.notif_action_pause
        }
        val togglePi = PendingIntent.getBroadcast(
            context,
            if (paused) 1 else 2,
            Intent(toggleAction.first).setPackage(context.packageName),
            piFlags
        )

        return NotificationCompat.Builder(context, CHANNEL_ID)
            .setContentTitle(title)
            .setContentText(content)
            .setSmallIcon(R.drawable.ic_stat_notify)
            .setOngoing(true)
            .setShowWhen(false)
            .setOnlyAlertOnce(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .setVisibility(NotificationCompat.VISIBILITY_SECRET)
            .setContentIntent(openPi)
            .addAction(0, context.getString(toggleAction.second), togglePi)
            .addAction(0, context.getString(R.string.notif_action_open), openPi)
            .setStyle(NotificationCompat.BigTextStyle().bigText(content))
            .build()
    }
}
