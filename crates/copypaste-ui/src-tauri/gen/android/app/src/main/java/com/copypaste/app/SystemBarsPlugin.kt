package com.copypaste.app

import android.app.Activity
import android.graphics.Color
import android.os.Build
import android.view.WindowManager
import android.webkit.WebView
import androidx.appcompat.app.AppCompatDelegate
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsControllerCompat
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

/** Keeps Android's edge-to-edge system bars and cutouts in the CSS inset tokens. */
@TauriPlugin
class SystemBarsPlugin(private val activity: Activity) : Plugin(activity) {
    private var webView: WebView? = null

    override fun load(webView: WebView) {
        super.load(webView)
        this.webView = webView
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            activity.window.attributes = activity.window.attributes.apply {
                layoutInDisplayCutoutMode =
                    WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
            }
        }
        // The activity installs the single inset owner before Wry starts its
        // first navigation. This plugin only replays that owner's state when
        // native appearance changes.
    }

    @Command
    fun setTheme(invoke: Invoke) {
        val light = invoke.getArgs().optString("theme", "dark") == "light"
        activity.runOnUiThread {
            AppCompatDelegate.setDefaultNightMode(
                if (light) AppCompatDelegate.MODE_NIGHT_NO
                else AppCompatDelegate.MODE_NIGHT_YES,
            )
            val window = activity.window
            window.statusBarColor = Color.TRANSPARENT
            window.navigationBarColor = Color.TRANSPARENT

            WindowInsetsControllerCompat(window, window.decorView).apply {
                isAppearanceLightStatusBars = light
                isAppearanceLightNavigationBars = light
            }
            webView?.let { view ->
                WebViewImeInsets.replayCss(view)
                ViewCompat.requestApplyInsets(view)
            }
        }
        invoke.resolve(JSObject())
    }

}
