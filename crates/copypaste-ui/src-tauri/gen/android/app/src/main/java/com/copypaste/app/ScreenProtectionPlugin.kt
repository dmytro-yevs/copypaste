package com.copypaste.app

import android.app.Activity
import android.util.Log
import android.view.WindowManager
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

/**
 * INV-35's Android half.
 *
 * `WebviewWindow::set_content_protected` reaches tao's
 * `set_content_protection`, which is compiled only for macOS and Windows — on
 * Android it does nothing at all, so the flag has to be set here.
 *
 * `FLAG_SECURE` blocks screenshots, screen recording and casting, and it also
 * blanks the app's thumbnail in the recents switcher. The last one is a
 * surprise worth knowing about and is why this is a setting rather than a
 * constant; it is on by default because a recents thumbnail of the clipboard
 * history is the same disclosure by another route.
 */
@TauriPlugin
class ScreenProtectionPlugin(private val activity: Activity) : Plugin(activity) {

    /**
     * The default is `true` on a malformed argument: the safe direction is the
     * one that keeps the window protected.
     *
     * Answered from inside the UI-thread block and only after the flag has been
     * read back, so the reply is what the window carries rather than what was
     * asked for. Resolving before the block ran told the caller a window was
     * protected while the flag was still queued.
     *
     * The read-back proves the window attribute, not that the compositor
     * honoured it; only a device can show the second, and INV-35's capture check
     * is what covers it.
     */
    @Command
    fun setProtected(invoke: Invoke) {
        val protect = invoke.getArgs().optBoolean("protected", true)
        // Window flags are UI-thread state. Already on it when the call arrives
        // from the WebView, in which case this runs inline.
        activity.runOnUiThread {
            val applied = try {
                if (protect) {
                    activity.window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
                } else {
                    activity.window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
                }
                isProtected() == protect
            } catch (e: Throwable) {
                Log.w(TAG, "the window protection flag could not be changed", e)
                false
            }
            if (applied) {
                invoke.resolve(JSObject().put("protected", protect))
            } else {
                invoke.reject("The window's screenshot protection could not be changed.")
            }
        }
    }

    private fun isProtected(): Boolean =
        (activity.window.attributes.flags and WindowManager.LayoutParams.FLAG_SECURE) != 0

    private companion object {
        const val TAG = "CopyPasteProtection"
    }
}
