package com.copypaste.app.widget

import android.content.Intent
import android.widget.RemoteViews
import android.widget.RemoteViewsService

/** Collection rows for the "Recent clips" widget on API 24–30. */
class RecentClipsWidgetService : RemoteViewsService() {
    override fun onGetViewFactory(intent: Intent): RemoteViewsFactory = EmptyRows

    private object EmptyRows : RemoteViewsFactory {
        override fun onCreate() = Unit
        override fun onDataSetChanged() = Unit
        override fun onDestroy() = Unit
        override fun getCount(): Int = 0
        override fun getViewAt(position: Int): RemoteViews? = null
        override fun getLoadingView(): RemoteViews? = null
        override fun getViewTypeCount(): Int = 1
        override fun getItemId(position: Int): Long = position.toLong()
        override fun hasStableIds(): Boolean = true
    }
}
