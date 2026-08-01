package com.copypaste.app

import android.app.Activity
import android.graphics.Color
import android.os.Build
import android.view.View
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
            val window = activity.window
            // The WebView supplies the colour below transparent edge-to-edge bars.
            window.statusBarColor = Color.TRANSPARENT
            window.navigationBarColor = Color.TRANSPARENT

            var flags = window.decorView.systemUiVisibility
            flags = if (light) {
                flags or View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR
            } else {
                flags and View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR.inv()
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                flags = if (light) {
                    flags or View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR
                } else {
                    flags and View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR.inv()
                }
            }
            window.decorView.systemUiVisibility = flags
        }
        invoke.resolve(JSObject())
    }

    @Command
    fun systemAccent(invoke: Invoke) {
        val accent = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            activity.getColor(android.R.color.system_accent1_500)
        } else {
            // This is the framework's theme accent on older Android releases.
            val attributes = activity.theme.obtainStyledAttributes(intArrayOf(android.R.attr.colorAccent))
            try {
                attributes.getColor(0, Color.rgb(99, 102, 241))
            } finally {
                attributes.recycle()
            }
        }
        invoke.resolve(JSObject().put("color", String.format("#%06x", 0xFFFFFF and accent)))
    }
}
