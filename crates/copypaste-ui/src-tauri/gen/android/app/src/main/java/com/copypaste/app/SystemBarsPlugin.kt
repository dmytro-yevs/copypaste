package com.copypaste.app

import android.app.Activity
import android.graphics.Color
import androidx.appcompat.app.AppCompatDelegate
import androidx.core.view.WindowInsetsControllerCompat
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

/** Keeps Android's edge-to-edge system bars legible in the selected app theme. */
@TauriPlugin
class SystemBarsPlugin(private val activity: Activity) : Plugin(activity) {
    @Command
    fun setTheme(invoke: Invoke) {
        val light = invoke.getArgs().optString("theme", "dark") == "light"
        activity.runOnUiThread {
            AppCompatDelegate.setDefaultNightMode(
                if (light) AppCompatDelegate.MODE_NIGHT_NO
                else AppCompatDelegate.MODE_NIGHT_YES,
            )
            val window = activity.window
            // The WebView supplies the colour below transparent edge-to-edge bars.
            window.statusBarColor = Color.TRANSPARENT
            window.navigationBarColor = Color.TRANSPARENT

            WindowInsetsControllerCompat(window, window.decorView).apply {
                isAppearanceLightStatusBars = light
                isAppearanceLightNavigationBars = light
            }
        }
        invoke.resolve(JSObject())
    }
}
