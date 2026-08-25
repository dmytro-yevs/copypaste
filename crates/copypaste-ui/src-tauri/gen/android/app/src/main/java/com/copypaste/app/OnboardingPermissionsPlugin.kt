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

@TauriPlugin
class OnboardingPermissionsPlugin(private val activity: android.app.Activity) : Plugin(activity) {
    private var lastTileAddResult: Int? = null

    @Command
    fun notificationFacts(invoke: Invoke) {
        invoke.resolve(notificationPayload())
    }

    @Command
    fun requestNotifications(invoke: Invoke) {
        (activity as MainActivity).requestNotificationPermission { facts ->
            invoke.resolve(
                CaptureBridgeJson.objectOf(NotificationPermissionFacts.serializer(), facts),
            )
        }
    }

    @Command
    fun tileFacts(invoke: Invoke) {
        invoke.resolve(tilePayload())
    }

    @Command
    fun requestTile(invoke: Invoke) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
            invoke.resolve(tilePayload())
            return
        }
        val manager = activity.getSystemService(StatusBarManager::class.java)
        manager.requestAddTileService(
            ComponentName(activity, CaptureTileService::class.java),
            activity.getString(R.string.capture_action),
            Icon.createWithResource(activity, R.drawable.ic_copypaste_capture_tile),
            activity.mainExecutor,
        ) { result ->
            lastTileAddResult = result
            invoke.resolve(tilePayload())
        }
    }

    @Command
    fun openNotificationSettings(invoke: Invoke) {
        val intent = Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS)
            .putExtra(Settings.EXTRA_APP_PACKAGE, activity.packageName)
        activity.startActivity(intent)
        invoke.resolve(JSObject())
    }

    private fun notificationPayload() = CaptureBridgeJson.objectOf(
        NotificationPermissionFacts.serializer(),
        (activity as MainActivity).notificationPermissionFacts(),
    )

    private fun tilePayload() = CaptureBridgeJson.objectOf(
        TilePermissionFacts.serializer(),
        TilePermissionFacts(
            apiLevel = Build.VERSION.SDK_INT,
            lastAddResult = lastTileAddResult,
            resultConstants = TileAddResultConstants.platform(),
        ),
    )
}
