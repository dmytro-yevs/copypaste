package com.copypaste.app

import android.app.Activity
import android.graphics.Color
import android.os.Build
import android.view.View
import android.view.WindowManager
import android.webkit.WebView
import androidx.appcompat.app.AppCompatDelegate
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import kotlin.math.roundToInt

/** Keeps Android's edge-to-edge system bars and cutouts in the CSS inset tokens. */
@TauriPlugin
class SystemBarsPlugin(private val activity: Activity) : Plugin(activity) {
    private var webView: WebView? = null
    private var lastInsets: InsetsPx? = null

    override fun load(webView: WebView) {
        super.load(webView)
        this.webView = webView
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            activity.window.attributes = activity.window.attributes.apply {
                layoutInDisplayCutoutMode =
                    WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
            }
        }
        ViewCompat.setOnApplyWindowInsetsListener(webView) { _, insets ->
            publishInsets(webView, insets)
            insets
        }
        ViewCompat.requestApplyInsets(webView)
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
                lastInsets?.let { publishCss(view, it) }
                ViewCompat.requestApplyInsets(view)
            }
        }
        invoke.resolve(JSObject())
    }

    private fun publishInsets(view: View, insets: WindowInsetsCompat) {
        val bars = insets.getInsets(
            WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout(),
        )
        val density = view.resources.displayMetrics.density.takeIf { it > 0f } ?: 1f
        val next = InsetsPx(
            top = cssPx(bars.top, density),
            right = cssPx(bars.right, density),
            bottom = cssPx(bars.bottom, density),
            left = cssPx(bars.left, density),
        )
        lastInsets = next
        val webView = this.webView ?: return
        publishCss(webView, next)
    }

    private fun publishCss(webView: WebView, insets: InsetsPx) {
        val script = """
            (function () {
              var root = document.documentElement;
              if (!root) return;
              root.style.setProperty('--inset-top', '${insets.top}px');
              root.style.setProperty('--inset-right', '${insets.right}px');
              root.style.setProperty('--inset-bottom', '${insets.bottom}px');
              root.style.setProperty('--inset-left', '${insets.left}px');
            })();
        """.trimIndent()
        webView.evaluateJavascript(script, null)
    }

    private fun cssPx(pixels: Int, density: Float): Int =
        (pixels.toFloat() / density).roundToInt().coerceAtLeast(0)

    private data class InsetsPx(
        val top: Int,
        val right: Int,
        val bottom: Int,
        val left: Int,
    )
}
