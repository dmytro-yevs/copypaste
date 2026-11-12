package com.copypaste.app.widget

import android.appwidget.AppWidgetManager
import android.content.Context
import android.content.Intent
import android.widget.RemoteViews
import android.widget.RemoteViewsService

/**
 * Collection rows for the "Recent clips" widget on API 24–30, where
 * `RemoteCollectionItems` is unavailable. Rows come from the last refresh; a
 * factory created after the process restarted asks for one instead of
 * reading the store on the launcher's binder call.
 */
class RecentClipsWidgetService : RemoteViewsService() {
    override fun onGetViewFactory(intent: Intent): RemoteViewsFactory = Rows(
        applicationContext,
        intent.getIntExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, AppWidgetManager.INVALID_APPWIDGET_ID),
    )

    private class Rows(private val context: Context, private val id: Int) : RemoteViewsFactory {
        private var rows: List<WidgetRow> = emptyList()

        override fun onCreate() = Unit

        override fun onDataSetChanged() {
            rows = WidgetRowsCache.get(id) ?: emptyList<WidgetRow>().also {
                WidgetHost.get(context).updater.refresh(intArrayOf(id))
            }
        }

        override fun onDestroy() {
            rows = emptyList()
        }

        override fun getCount(): Int = rows.size

        override fun getViewAt(position: Int): RemoteViews? =
            rows.getOrNull(position)?.let { WidgetViews.row(context, it, System.currentTimeMillis()) }

        override fun getLoadingView(): RemoteViews? = null

        override fun getViewTypeCount(): Int = 1

        override fun getItemId(position: Int): Long = rows.getOrNull(position)?.let(WidgetViews::stableId) ?: position.toLong()

        override fun hasStableIds(): Boolean = true
    }
}
