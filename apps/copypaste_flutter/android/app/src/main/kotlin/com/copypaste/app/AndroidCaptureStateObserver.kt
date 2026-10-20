package com.copypaste.app

import android.app.AppOpsManager
import android.content.Context

/** Observes the system-owned capture prerequisites while the Flutter stream has a listener. */
internal class AndroidCaptureStateObserver(
    context: Context,
    private val changed: () -> Unit,
) {
    private val app = context.applicationContext
    private val appOps = app.getSystemService(AppOpsManager::class.java)
    private var observing = false
    private val appOpListener = AppOpsManager.OnOpChangedListener { operation, packageName ->
        if (packageName == app.packageName && operation in observedAppOps) changed()
    }

    fun start() {
        if (observing) return
        observing = true
        val manager = appOps ?: return
        for (operation in observedAppOps) {
            runCatching { manager.startWatchingMode(operation, app.packageName, appOpListener) }
        }
    }

    fun stop() {
        if (!observing) return
        observing = false
        appOps?.let { manager -> runCatching { manager.stopWatchingMode(appOpListener) } }
    }

    companion object {
        private val observedAppOps = setOf(
            "android:run_in_background",
            "android:run_any_in_background",
            AppOpsManager.OPSTR_SYSTEM_ALERT_WINDOW,
        )
    }
}
