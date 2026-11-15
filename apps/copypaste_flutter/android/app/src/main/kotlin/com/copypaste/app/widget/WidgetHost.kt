package com.copypaste.app.widget

import android.content.Context
import com.copypaste.app.RuntimeAccessWorker

/** The process-wide widget surface and updater, created on first use. */
internal class WidgetHost private constructor(context: Context) {
    val surface = LauncherWidgetSurface(context.applicationContext)
    val runtime = RuntimeAccessWorker.native(context)
    val updater = WidgetUpdater(surface, runtime, HandlerWidgetTimer())

    companion object {
        @Volatile
        private var instance: WidgetHost? = null

        fun get(context: Context): WidgetHost =
            instance ?: synchronized(this) { instance ?: WidgetHost(context).also { instance = it } }
    }
}
