package com.copypaste.app.widget

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.UserManager
import android.text.format.DateUtils
import android.view.View
import android.widget.RemoteViews
import com.copypaste.app.AndroidCaptureState
import com.copypaste.app.QuickAccessActivity
import com.copypaste.app.R
import com.copypaste.app.RecentClipsWidgetProvider
import java.text.DateFormat
import java.util.concurrent.ConcurrentHashMap

internal object WidgetIntents {
    /** A row tap, handled by [QuickAccessActivity] through [WidgetTapHandler]. */
    const val ACTION_TAP = "com.copypaste.app.widget.TAP"

    /** The header Save action; [QuickAccessActivity] reads the clipboard with focus. */
    const val ACTION_SAVE_CLIPBOARD = "com.copypaste.app.action.SAVE_CLIPBOARD"
    const val ACTION_CYCLE_FILTER = "com.copypaste.app.widget.CYCLE_FILTER"
    const val ACTION_CYCLE_PRIVACY = "com.copypaste.app.widget.CYCLE_PRIVACY"
    const val ACTION_REFRESH = "com.copypaste.app.widget.REFRESH"
    const val ACTION_RESUME = "com.copypaste.app.widget.RESUME"
    const val EXTRA_ITEM_ID = "item-id"
    const val EXTRA_SNIPPET = "snippet"
    const val EXTRA_TAP = "tap"
}

/** Rows handed to [RecentClipsWidgetService] on API 24–30; process memory only (W5). */
internal object WidgetRowsCache {
    private val rows = ConcurrentHashMap<Int, List<WidgetRow>>()

    fun get(id: Int): List<WidgetRow>? = rows[id]
    fun put(id: Int, value: List<WidgetRow>) = rows.put(id, value)
    fun remove(ids: IntArray) = ids.forEach { rows.remove(it) }
}

/** The launcher-facing [WidgetSurface]: AppWidgetManager plus RemoteViews. */
internal class LauncherWidgetSurface(private val context: Context) : WidgetSurface {
    private val manager = AppWidgetManager.getInstance(context)
    private val component = ComponentName(context, RecentClipsWidgetProvider::class.java)
    private val configs = WidgetConfigStore(context)

    @Volatile
    private var ids: IntArray? = null

    /** Read once per process and after the provider sees instances change. */
    override fun instances(): IntArray = ids ?: manager.getAppWidgetIds(component).also { ids = it }

    fun instancesChanged() {
        ids = null
    }

    override fun config(id: Int): WidgetConfig = configs.get(id)

    fun updateConfig(id: Int, change: (WidgetConfig) -> WidgetConfig) = configs.put(id, change(configs.get(id)))

    fun forget(ids: IntArray) {
        configs.remove(ids)
        WidgetRowsCache.remove(ids)
        instancesChanged()
    }

    override fun rowLimit(id: Int): Int = WidgetViews.rowLimit(
        manager.getAppWidgetOptions(id).getInt(AppWidgetManager.OPTION_APPWIDGET_MAX_HEIGHT, 0),
        context.resources.configuration.fontScale,
    )

    override fun storeReadable(): Boolean =
        context.getSystemService(UserManager::class.java)?.isUserUnlocked != false

    override fun show(id: Int, config: WidgetConfig, snapshot: WidgetSnapshot) {
        manager.updateAppWidget(id, WidgetViews.frame(context, id, config, snapshot))
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) {
            @Suppress("DEPRECATION")
            manager.notifyAppWidgetViewDataChanged(id, R.id.widget_list)
        }
    }
}

internal object WidgetViews {
    /** Rows shown at most; a taller widget shows no more (W1 cap). */
    const val MAX_ROWS = 5
    private const val CHROME_DP = 64
    private const val ROW_DP = 56

    /** Fewer rows in a short widget or with large fonts, never cut labels (spec 5.7). */
    fun rowLimit(heightDp: Int, fontScale: Float): Int {
        if (heightDp <= 0) return MAX_ROWS
        return ((heightDp - CHROME_DP) / (ROW_DP * fontScale.coerceAtLeast(1f))).toInt().coerceIn(1, MAX_ROWS)
    }

    fun frame(context: Context, id: Int, config: WidgetConfig, snapshot: WidgetSnapshot): RemoteViews {
        val views = RemoteViews(context.packageName, R.layout.widget_recent_clips)
        val filter = context.getString(filterLabel(config.filter))
        views.setTextViewText(R.id.widget_filter, filter)
        views.setContentDescription(R.id.widget_filter, context.getString(R.string.widget_filter_description, filter))
        views.setOnClickPendingIntent(R.id.widget_filter, broadcast(context, id, WidgetIntents.ACTION_CYCLE_FILTER))
        views.setImageViewResource(
            R.id.widget_privacy,
            if (config.privacy == WidgetPrivacy.Full) R.drawable.ic_lucide_eye else R.drawable.ic_lucide_eye_off,
        )
        views.setContentDescription(
            R.id.widget_privacy,
            context.getString(R.string.widget_privacy_description, context.getString(privacyLabel(config.privacy))),
        )
        views.setOnClickPendingIntent(R.id.widget_privacy, broadcast(context, id, WidgetIntents.ACTION_CYCLE_PRIVACY))
        openApp(context)?.let { views.setOnClickPendingIntent(R.id.widget_search, it) }
        views.setOnClickPendingIntent(
            R.id.widget_save,
            quickAccess(context, id, Intent(WidgetIntents.ACTION_SAVE_CLIPBOARD), mutable = false),
        )

        val paused = (snapshot as? WidgetSnapshot.Rows)?.paused ?: (snapshot as? WidgetSnapshot.Count)?.paused ?: false
        views.setViewVisibility(R.id.widget_banner, if (paused) View.VISIBLE else View.GONE)
        if (paused) views.setOnClickPendingIntent(R.id.widget_banner, broadcast(context, id, WidgetIntents.ACTION_RESUME))

        val rows = (snapshot as? WidgetSnapshot.Rows)?.rows.orEmpty()
        list(context, id, views, rows)
        val state = stateText(context, config, snapshot)
        views.setViewVisibility(R.id.widget_state, if (state == null) View.GONE else View.VISIBLE)
        views.setViewVisibility(R.id.widget_list, if (state == null) View.VISIBLE else View.GONE)
        if (state != null) {
            views.setTextViewText(R.id.widget_state, state)
            views.setContentDescription(R.id.widget_state, state)
            stateAction(context, id, config, snapshot)?.let { views.setOnClickPendingIntent(R.id.widget_state, it) }
        }
        return views
    }

    /** Every frame replaces the list, so cleared rows leave launcher memory too. */
    private fun list(context: Context, id: Int, views: RemoteViews, rows: List<WidgetRow>) {
        views.setPendingIntentTemplate(
            R.id.widget_list,
            quickAccess(context, id, Intent(WidgetIntents.ACTION_TAP), mutable = true),
        )
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            val items = RemoteViews.RemoteCollectionItems.Builder().setHasStableIds(true).setViewTypeCount(1)
            val now = System.currentTimeMillis()
            rows.forEach { items.addItem(stableId(it), row(context, it, now)) }
            views.setRemoteAdapter(R.id.widget_list, items.build())
        } else {
            WidgetRowsCache.put(id, rows)
            val intent = Intent(context, RecentClipsWidgetService::class.java)
                .putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, id)
            intent.data = Uri.parse(intent.toUri(Intent.URI_INTENT_SCHEME))
            @Suppress("DEPRECATION")
            views.setRemoteAdapter(R.id.widget_list, intent)
        }
    }

    fun stableId(row: WidgetRow): Long = (if (row.snippet) "s:" else "c:").plus(row.id).hashCode().toLong()

    fun row(context: Context, row: WidgetRow, nowMs: Long): RemoteViews {
        val views = RemoteViews(context.packageName, R.layout.widget_row)
        val kind = context.getString(kindLabel(row.kind))
        val title = when {
            row.masked && !row.snippet -> context.getString(R.string.widget_secret_item)
            else -> row.text ?: kind
        }
        val detail = if (row.snippet) {
            if (row.masked) context.getString(R.string.widget_concealed_snippet) else row.secondary.orEmpty()
        } else {
            val time = DateUtils.formatSameDayTime(row.createdAtMs, nowMs, DateFormat.SHORT, DateFormat.SHORT)
            listOfNotNull(row.sourceApp, time).joinToString(" · ")
        }
        views.setTextViewText(R.id.widget_row_title, title)
        views.setTextViewText(R.id.widget_row_detail, detail)
        views.setViewVisibility(R.id.widget_row_detail, if (detail.isEmpty()) View.GONE else View.VISIBLE)
        val icon = when {
            row.masked -> R.drawable.ic_lucide_lock
            row.pinned -> R.drawable.ic_lucide_pin
            else -> null
        }
        views.setViewVisibility(R.id.widget_row_icon, if (icon == null) View.GONE else View.VISIBLE)
        icon?.let { views.setImageViewResource(R.id.widget_row_icon, it) }
        views.setContentDescription(
            R.id.widget_row,
            if (row.masked) {
                context.getString(R.string.widget_secret_description, if (row.snippet) title else kind)
            } else {
                listOf(title, detail).filter { it.isNotEmpty() }.joinToString(", ")
            },
        )
        views.setOnClickFillInIntent(
            R.id.widget_row,
            Intent()
                .putExtra(WidgetIntents.EXTRA_ITEM_ID, row.id)
                .putExtra(WidgetIntents.EXTRA_SNIPPET, row.snippet)
                .putExtra(WidgetIntents.EXTRA_TAP, row.tap.name),
        )
        return views
    }

    private fun stateText(context: Context, config: WidgetConfig, snapshot: WidgetSnapshot): String? =
        when (snapshot) {
            WidgetSnapshot.Unavailable -> context.getString(R.string.widget_unavailable)
            WidgetSnapshot.Failed -> context.getString(R.string.widget_error)
            is WidgetSnapshot.Count -> snapshot.count?.let { count ->
                val plural = if (config.filter == WidgetFilter.Snippets) R.plurals.widget_hidden_snippets else R.plurals.widget_hidden_clips
                context.resources.getQuantityString(plural, count.coerceAtMost(Int.MAX_VALUE.toLong()).toInt(), count)
            } ?: context.getString(R.string.widget_hidden)
            is WidgetSnapshot.Rows -> when {
                snapshot.rows.isNotEmpty() -> null
                config.filter == WidgetFilter.Pinned -> context.getString(R.string.widget_empty_pinned)
                config.filter == WidgetFilter.Snippets -> context.getString(R.string.widget_empty_snippets)
                setupNeeded(context) -> context.getString(R.string.widget_setup)
                else -> context.getString(R.string.widget_empty)
            }
        }

    private fun stateAction(context: Context, id: Int, config: WidgetConfig, snapshot: WidgetSnapshot): PendingIntent? =
        when {
            snapshot == WidgetSnapshot.Failed -> broadcast(context, id, WidgetIntents.ACTION_REFRESH)
            snapshot is WidgetSnapshot.Rows && config.filter == WidgetFilter.History && setupNeeded(context) ->
                openApp(context)
            else -> null
        }

    /** No capture tier is set up: copies reach History only from the app, panel or share. */
    private fun setupNeeded(context: Context): Boolean =
        !AndroidCaptureState.captureEnabled(context) && !AndroidCaptureState.foregroundCaptureEnabled(context)

    private fun broadcast(context: Context, id: Int, action: String): PendingIntent = PendingIntent.getBroadcast(
        context,
        id,
        Intent(context, RecentClipsWidgetProvider::class.java)
            .setAction(action)
            .putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, id),
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
    )

    /** Row taps fill the template in, so only that PendingIntent is mutable. */
    private fun quickAccess(context: Context, id: Int, intent: Intent, mutable: Boolean): PendingIntent {
        intent.setClass(context, QuickAccessActivity::class.java).addFlags(
            Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_NO_ANIMATION or
                Intent.FLAG_ACTIVITY_EXCLUDE_FROM_RECENTS,
        )
        val mutability = if (mutable && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            PendingIntent.FLAG_MUTABLE
        } else if (mutable) {
            0
        } else {
            PendingIntent.FLAG_IMMUTABLE
        }
        return PendingIntent.getActivity(context, id, intent, PendingIntent.FLAG_UPDATE_CURRENT or mutability)
    }

    private fun openApp(context: Context): PendingIntent? =
        context.packageManager.getLaunchIntentForPackage(context.packageName)?.let {
            PendingIntent.getActivity(context, 0, it, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        }

    private fun filterLabel(filter: WidgetFilter): Int = when (filter) {
        WidgetFilter.History -> R.string.widget_filter_history
        WidgetFilter.Pinned -> R.string.widget_filter_pinned
        WidgetFilter.Snippets -> R.string.widget_filter_snippets
    }

    private fun privacyLabel(privacy: WidgetPrivacy): Int = when (privacy) {
        WidgetPrivacy.Full -> R.string.widget_privacy_full
        WidgetPrivacy.Titles -> R.string.widget_privacy_titles
        WidgetPrivacy.Hidden -> R.string.widget_privacy_hidden
    }

    private fun kindLabel(kind: WidgetKind): Int = when (kind) {
        WidgetKind.Text -> R.string.widget_kind_text
        WidgetKind.Link -> R.string.widget_kind_link
        WidgetKind.Image -> R.string.widget_kind_image
        WidgetKind.File -> R.string.widget_kind_file
        WidgetKind.Color -> R.string.widget_kind_color
        WidgetKind.Code -> R.string.widget_kind_code
        WidgetKind.Snippet -> R.string.widget_kind_snippet
    }
}
