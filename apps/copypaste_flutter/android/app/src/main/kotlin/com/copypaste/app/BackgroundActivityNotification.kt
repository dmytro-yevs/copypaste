package com.copypaste.app

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.Build
import android.text.format.DateUtils
import androidx.core.app.NotificationCompat
import com.copypaste.app.quickaccess.CaptureTileSnapshot
import com.copypaste.app.quickaccess.QuickAccessRuntime
import com.copypaste.app.quickaccess.QuickAccessTileStore
import com.copypaste.app.quickaccess.QuickAccessTiles
import com.copypaste.app.quickaccess.pausedOf
import java.text.DateFormat
import org.json.JSONObject

/** What the shared background notification reports; it never carries clip content. */
internal data class BackgroundNotificationState(
    /** Null until the runtime reported its pause state. */
    val paused: Boolean?,
    val captureRunning: Boolean,
    val syncRunning: Boolean,
    val lastSyncAt: Long = 0L,
    val lastSavedAt: Long = 0L,
)

internal data class BackgroundNotificationContent(
    val title: String,
    val text: String,
    /** Label of the pause toggle; null while the pause state is unknown. */
    val pauseAction: String?,
)

internal fun backgroundNotificationContent(
    context: Context,
    state: BackgroundNotificationState,
    now: Long,
): BackgroundNotificationContent {
    fun time(at: Long) = DateUtils.formatSameDayTime(at, now, DateFormat.SHORT, DateFormat.SHORT).toString()
    val parts = buildList {
        if (state.captureRunning && state.paused != true) add(context.getString(R.string.background_status_capturing))
        if (state.syncRunning) add(context.getString(R.string.background_status_syncing))
        when {
            state.lastSyncAt > 0 -> add(context.getString(R.string.background_status_last_sync, time(state.lastSyncAt)))
            state.lastSavedAt > 0 -> add(context.getString(R.string.background_status_last_saved, time(state.lastSavedAt)))
        }
    }
    return BackgroundNotificationContent(
        title = context.getString(
            if (state.paused == true) R.string.background_title_paused else R.string.background_title,
        ),
        text = parts.joinToString(" · ").ifEmpty { context.getString(R.string.background_text) },
        pauseAction = state.paused?.let {
            context.getString(if (it) R.string.background_action_resume else R.string.background_action_pause)
        },
    )
}

/** Newest successful peer sync in a runtime `status` reply, or 0. */
internal fun lastSyncAt(status: JSONObject?): Long {
    val peers = status?.optJSONObject("sync_status")?.optJSONArray("peers") ?: return 0L
    return (0 until peers.length()).maxOfOrNull { peers.optJSONObject(it)?.optLong("last_success_ms", 0L) ?: 0L } ?: 0L
}

/** One notification shared by every CopyPaste foreground service. */
internal object BackgroundActivityNotification {
    // Keep the clipboard channel and ID to preserve existing channel preferences.
    private const val channelId = "clipboard-capture"
    private const val notificationId = 2207
    internal const val ACTION_TOGGLE_PAUSE = "com.copypaste.app.action.TOGGLE_PAUSE"

    fun start(service: Service) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            service.getSystemService(NotificationManager::class.java).createNotificationChannel(
                NotificationChannel(
                    channelId,
                    service.getString(R.string.background_channel_name),
                    NotificationManager.IMPORTANCE_LOW,
                ).apply {
                    description = service.getString(R.string.background_channel_description)
                    setSound(null, null)
                    enableVibration(false)
                },
            )
        }
        // Android retains this shared ID until the last foreground service stops.
        // Do not cancel it from an individual service's shutdown path.
        service.startForeground(notificationId, build(service, cachedState(service)))
        refresh(service)
    }

    /** Re-reads pause and sync state and redraws the notification while it is shown. */
    fun refresh(context: Context) {
        val app = context.applicationContext
        if (!shown(app)) return
        runCatching {
            QuickAccessRuntime.worker(app).request("status") { reply ->
                val data = (reply as? RuntimeReply.Ok)?.objectData()
                val state = cachedState(app).copy(
                    paused = pausedOf(reply) ?: cachedState(app).paused,
                    lastSyncAt = lastSyncAt(data),
                )
                if (shown(app)) runCatching { manager(app)?.notify(notificationId, build(app, state)) }
            }
        }
    }

    private fun cachedState(context: Context): BackgroundNotificationState {
        val store = QuickAccessTileStore(context)
        return BackgroundNotificationState(
            paused = store.capture.paused.takeIf { store.knowsCapture },
            captureRunning = ClipboardCaptureService.isRunning(),
            syncRunning = BackgroundSyncService.isRunning(),
            lastSavedAt = maxOf(AndroidCaptureState.lastCaptureAt(context), store.lastSavedAt),
        )
    }

    private fun build(context: Context, state: BackgroundNotificationState) =
        backgroundNotificationContent(context, state, System.currentTimeMillis()).let { content ->
            NotificationCompat.Builder(context, channelId)
                .setSmallIcon(R.mipmap.ic_launcher)
                .setContentTitle(content.title)
                .setContentText(content.text)
                .setOngoing(true)
                .setOnlyAlertOnce(true)
                .setSilent(true)
                .setPriority(NotificationCompat.PRIORITY_LOW)
                .setContentIntent(openIntent(context))
                .apply {
                    content.pauseAction?.let { label ->
                        addAction(0, label, broadcast(context, ACTION_TOGGLE_PAUSE))
                    }
                }
                .addAction(
                    0,
                    context.getString(R.string.background_action_send),
                    PendingIntent.getActivity(
                        context,
                        1,
                        QuickAccessActivity.saveClipboardIntent(context),
                        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
                    ),
                )
                .addAction(0, context.getString(R.string.background_action_open), openIntent(context))
                .build()
        }

    private fun openIntent(context: Context) = PendingIntent.getActivity(
        context,
        0,
        Intent(context, MainActivity::class.java),
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
    )

    private fun broadcast(context: Context, action: String) = PendingIntent.getBroadcast(
        context,
        2,
        Intent(context, BackgroundNotificationActionReceiver::class.java).setAction(action),
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
    )

    private fun manager(context: Context) = context.getSystemService(NotificationManager::class.java)

    private fun shown(context: Context): Boolean = runCatching {
        manager(context)?.activeNotifications?.any { it.id == notificationId } == true
    }.getOrDefault(false)
}

/** Handles the notification's pause toggle without opening an activity. */
class BackgroundNotificationActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != BackgroundActivityNotification.ACTION_TOGGLE_PAUSE) return
        val app = context.applicationContext
        val result = goAsync()
        val worker = QuickAccessRuntime.worker(app)
        worker.request("status") { status ->
            val paused = pausedOf(status)
            if (paused == null) {
                result.finish()
                return@request
            }
            worker.request("set_private_mode", JSONObject().put("enabled", !paused)) { reply ->
                // Publishing a changed pause state also redraws this notification.
                pausedOf(reply)?.let { applied ->
                    QuickAccessTiles.publish(app, CaptureTileSnapshot(applied, QuickAccessTiles.currentTier(app)))
                    AndroidCaptureSignals.changed()
                }
                result.finish()
            }
        }
    }
}
