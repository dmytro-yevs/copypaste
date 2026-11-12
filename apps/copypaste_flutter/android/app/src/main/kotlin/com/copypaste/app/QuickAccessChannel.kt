package com.copypaste.app

import android.app.Activity
import android.app.StatusBarManager
import android.appwidget.AppWidgetManager
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.graphics.drawable.Icon
import android.os.Build
import android.provider.Settings
import android.view.inputmethod.InputMethodManager
import io.flutter.plugin.common.BinaryMessenger
import io.flutter.plugin.common.MethodChannel

/** Serves the Quick access settings: keyboard panel setup, tile and widget placement, panel options. */
internal class QuickAccessChannel(private val activity: Activity, messenger: BinaryMessenger) {
    private val channel = MethodChannel(messenger, "com.copypaste.app/quick_access")
    private var disposed = false

    init {
        channel.setMethodCallHandler { call, result ->
            when (call.method) {
                "state" -> result.success(QuickAccessStatus.read(activity))
                "openKeyboardSettings" -> result.success(launch(Intent(Settings.ACTION_INPUT_METHOD_SETTINGS)))
                "showKeyboardPicker" -> result.success(showKeyboardPicker())
                "requestAddTile" -> {
                    val tile = QuickAccessTile.fromWire(call.argument<String>("tile"))
                    if (tile == null) result.error("invalid_arguments", null, null)
                    else requestAddTile(tile) { result.success(it) }
                }
                "requestPinWidget" -> result.success(requestPinWidget())
                "setPanelOption" -> {
                    val option = QuickAccessPreferences.PanelOption.fromWire(call.argument<String>("option"))
                    val enabled = call.argument<Boolean>("enabled")
                    if (option == null || enabled == null) result.error("invalid_arguments", null, null)
                    else if (!QuickAccessPreferences.setEnabled(activity, option, enabled)) {
                        result.error("write_failed", null, null)
                    } else result.success(QuickAccessStatus.read(activity))
                }
                else -> result.notImplemented()
            }
        }
    }

    fun dispose() {
        disposed = true
        channel.setMethodCallHandler(null)
    }

    private fun showKeyboardPicker(): Boolean {
        if (!QuickAccessStatus.keyboardEnabled(activity)) return false
        val manager = activity.getSystemService(InputMethodManager::class.java) ?: return false
        manager.showInputMethodPicker()
        return true
    }

    private fun requestAddTile(tile: QuickAccessTile, complete: (String) -> Unit) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return complete(TILE_UNAVAILABLE)
        val manager = activity.getSystemService(StatusBarManager::class.java)
            ?: return complete(TILE_UNAVAILABLE)
        try {
            manager.requestAddTileService(
                ComponentName(activity, tile.service),
                activity.getString(tile.label),
                Icon.createWithResource(activity, tile.icon),
                activity.mainExecutor,
            ) { code -> if (!disposed) complete(tileRequestOutcome(code)) }
        } catch (_: RuntimeException) {
            complete(TILE_UNAVAILABLE)
        }
    }

    private fun requestPinWidget(): Boolean {
        if (!QuickAccessStatus.widgetPinSupported(activity)) return false
        return try {
            AppWidgetManager.getInstance(activity).requestPinAppWidget(
                ComponentName(activity, RecentClipsWidgetProvider::class.java),
                null,
                null,
            )
        } catch (_: RuntimeException) {
            false
        }
    }

    private fun launch(intent: Intent): Boolean = try {
        if (intent.resolveActivity(activity.packageManager) == null) false
        else {
            activity.startActivity(intent)
            true
        }
    } catch (_: RuntimeException) {
        false
    }
}

internal enum class QuickAccessTile(val wire: String, val service: Class<*>, val label: Int, val icon: Int) {
    SAVE_CLIPBOARD(
        "saveClipboard",
        SaveClipboardTileService::class.java,
        R.string.quick_access_tile_save_label,
        R.drawable.ic_lucide_clipboard,
    ),
    CAPTURE_TOGGLE(
        "captureToggle",
        CaptureToggleTileService::class.java,
        R.string.quick_access_tile_capture_label,
        R.drawable.ic_lucide_eye,
    );

    companion object {
        fun fromWire(wire: String?): QuickAccessTile? = entries.firstOrNull { it.wire == wire }
    }
}

internal const val TILE_ADDED = "added"
internal const val TILE_ALREADY_ADDED = "alreadyAdded"
internal const val TILE_NOT_ADDED = "notAdded"
internal const val TILE_UNAVAILABLE = "unavailable"

/** Maps a StatusBarManager tile request result; every error code means the prompt could not be shown. */
internal fun tileRequestOutcome(code: Int): String = when (code) {
    StatusBarManager.TILE_ADD_REQUEST_RESULT_TILE_ADDED -> TILE_ADDED
    StatusBarManager.TILE_ADD_REQUEST_RESULT_TILE_ALREADY_ADDED -> TILE_ALREADY_ADDED
    StatusBarManager.TILE_ADD_REQUEST_RESULT_TILE_NOT_ADDED -> TILE_NOT_ADDED
    else -> TILE_UNAVAILABLE
}

/** Reads quick access state on demand; nothing here is polled or observed. */
internal object QuickAccessStatus {
    fun read(context: Context): Map<String, Any> = mapOf(
        "keyboardEnabled" to keyboardEnabled(context),
        "keyboardSelected" to keyboardSelected(context),
        "backgroundCapture" to backgroundCapture(context),
        "tileRequestSupported" to (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU),
        "widgetPinSupported" to widgetPinSupported(context),
        "widgetCount" to widgetCount(context),
        "panelOptions" to QuickAccessPreferences.asMap(context),
    )

    private fun keyboard(context: Context) = ComponentName(context, ClipboardInputMethodService::class.java)

    fun keyboardEnabled(context: Context): Boolean =
        context.getSystemService(InputMethodManager::class.java)
            ?.enabledInputMethodList
            ?.any { it.component == keyboard(context) } == true

    fun keyboardSelected(context: Context): Boolean {
        val current = Settings.Secure.getString(context.contentResolver, Settings.Secure.DEFAULT_INPUT_METHOD)
            ?.let(ComponentName::unflattenFromString)
        return current == keyboard(context)
    }

    private fun backgroundCapture(context: Context): Boolean =
        AndroidCaptureState.privilegedGrants(context) &&
            AndroidCaptureState.notificationGranted(context) &&
            AndroidCaptureState.captureEnabled(context) &&
            ClipboardCaptureService.isRunning()

    fun widgetPinSupported(context: Context): Boolean =
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
            AppWidgetManager.getInstance(context)?.isRequestPinAppWidgetSupported == true

    private fun widgetCount(context: Context): Int =
        AppWidgetManager.getInstance(context)
            ?.getAppWidgetIds(ComponentName(context, RecentClipsWidgetProvider::class.java))
            ?.size ?: 0
}
