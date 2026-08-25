package com.copypaste.app

import android.app.StatusBarManager
import android.content.ComponentName
import android.content.Intent
import android.graphics.drawable.Icon
import android.os.Build
import android.provider.Settings
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.util.concurrent.atomic.AtomicReference

@TauriPlugin
class OnboardingPermissionsPlugin(private val activity: android.app.Activity) : Plugin(activity) {
    private val pendingNotifications = AtomicReference<Invoke?>(null)
    private var tileStatus: String =
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) "unavailable" else "prompt"
    private val prefs by lazy {
        activity.getSharedPreferences("onboarding-permissions", android.content.Context.MODE_PRIVATE)
    }

    @Command
    fun notificationFacts(invoke: Invoke) {
        invoke.resolve(notificationPayload())
    }

    @Command
    fun requestNotifications(invoke: Invoke) {
        if (CaptureNotifications.isPermissionGranted(activity)) {
            markAsked()
            invoke.resolve(notificationPayload())
            return
        }
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
            invoke.resolve(notificationPayload())
            return
        }
        pendingNotifications.getAndSet(invoke)?.reject("Another notification request is already open.")
        markAsked()
        (activity as MainActivity).requestNotificationPermission { _ ->
            pendingNotifications.getAndSet(null)?.resolve(notificationPayload())
        }
    }

    @Command
    fun tileFacts(invoke: Invoke) {
        invoke.resolve(JSObject().put("status", tileStatus))
    }

    @Command
    fun requestTile(invoke: Invoke) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
            tileStatus = "unavailable"
            invoke.resolve(JSObject().put("result", -1))
            return
        }
        val manager = activity.getSystemService(StatusBarManager::class.java)
        manager.requestAddTileService(
            ComponentName(activity, CaptureTileService::class.java),
            activity.getString(R.string.capture_action),
            Icon.createWithResource(activity, R.drawable.ic_copypaste_capture_tile),
            activity.mainExecutor,
        ) { result ->
            tileStatus = TileAddGate.status(result)
            invoke.resolve(JSObject().put("result", result))
        }
    }

    @Command
    fun openNotificationSettings(invoke: Invoke) {
        val intent = Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS)
            .putExtra(Settings.EXTRA_APP_PACKAGE, activity.packageName)
        activity.startActivity(intent)
        invoke.resolve(JSObject())
    }

    private fun markAsked() {
        prefs.edit().putBoolean(ASKED_NOTIFICATIONS, true).apply()
    }

    private fun notificationPayload(): JSObject {
        val granted = CaptureNotifications.isPermissionGranted(activity)
        val everAsked = prefs.getBoolean(ASKED_NOTIFICATIONS, false)
        val rationale = Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            activity.shouldShowRequestPermissionRationale(
                android.Manifest.permission.POST_NOTIFICATIONS,
            )
        return JSObject()
            .put("apiLevel", Build.VERSION.SDK_INT)
            .put("granted", granted)
            .put("everAsked", everAsked)
            .put("showRationale", rationale)
    }

    private companion object {
        const val ASKED_NOTIFICATIONS = "asked_notifications"
    }
}
