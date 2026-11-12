package com.copypaste.app

import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.Context
import android.content.Intent
import android.os.Bundle
import com.copypaste.app.widget.WidgetHost
import com.copypaste.app.widget.WidgetIntents
import org.json.JSONObject

/**
 * Home-screen "Recent clips" widget. It has no periodic updates: the system
 * asks for a first frame, header actions change one instance, and runtime
 * changes arrive through [notifyChanged].
 */
class RecentClipsWidgetProvider : AppWidgetProvider() {
    override fun onUpdate(context: Context, manager: AppWidgetManager, ids: IntArray) {
        val host = WidgetHost.get(context)
        host.surface.instancesChanged()
        val pending = goAsync()
        host.updater.refresh(ids) { pending.finish() }
    }

    override fun onAppWidgetOptionsChanged(context: Context, manager: AppWidgetManager, id: Int, options: Bundle) {
        val pending = goAsync()
        WidgetHost.get(context).updater.refresh(intArrayOf(id)) { pending.finish() }
    }

    override fun onDeleted(context: Context, ids: IntArray) = WidgetHost.get(context).surface.forget(ids)

    override fun onDisabled(context: Context) = WidgetHost.get(context).surface.instancesChanged()

    override fun onReceive(context: Context, intent: Intent) {
        val id = intent.getIntExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, AppWidgetManager.INVALID_APPWIDGET_ID)
        val host = WidgetHost.get(context)
        when (intent.action) {
            WidgetIntents.ACTION_CYCLE_FILTER -> host.surface.updateConfig(id) { it.copy(filter = it.filter.next()) }
            WidgetIntents.ACTION_CYCLE_PRIVACY -> host.surface.updateConfig(id) { it.copy(privacy = it.privacy.next()) }
            WidgetIntents.ACTION_REFRESH -> Unit
            WidgetIntents.ACTION_RESUME -> return resume(host)
            else -> return super.onReceive(context, intent)
        }
        if (id == AppWidgetManager.INVALID_APPWIDGET_ID) return
        val pending = goAsync()
        host.updater.refresh(intArrayOf(id)) { pending.finish() }
    }

    /** The paused banner's Resume: capture is on again for every surface. */
    private fun resume(host: WidgetHost) {
        val pending = goAsync()
        host.runtime.request("set_private_mode", JSONObject().put("enabled", false)) {
            host.updater.refresh(host.surface.instances()) { pending.finish() }
        }
    }

    companion object {
        /**
         * Called by the runtime, on any thread, when what the widget shows
         * changed. [cleared] (history cleared, device removed) empties every
         * instance immediately; other changes are coalesced. Carries no content.
         */
        @JvmStatic
        fun notifyChanged(context: Context, cleared: Boolean) {
            val updater = WidgetHost.get(context).updater
            if (cleared) updater.cleared() else updater.changed()
        }
    }
}
