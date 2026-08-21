package com.copypaste.app

import android.os.Build
import android.util.Log
import rikka.shizuku.Shizuku

/**
 * Shizuku is only the rung-2 setup and settings bridge.
 *
 * The live clipboard reader is app-owned (`ClipCascadeCapture` +
 * `ClipboardFloatingActivity` + `CaptureService`). Shizuku remains for:
 * - checking whether its server is available;
 * - requesting our one-time permission; and
 * - calling the user service that applies ClipCascade-style grants and the
 *   optional clipboard-notice setting.
 */
object ShizukuClipboard {
    private const val TAG = "CopyPasteShizuku"

    @Volatile
    var lastFailure: String? = null
        private set

    fun isRunning(): Boolean = try {
        Shizuku.pingBinder()
    } catch (_: Throwable) {
        false
    }

    fun hasPermission(): Boolean = try {
        Shizuku.checkSelfPermission() == android.content.pm.PackageManager.PERMISSION_GRANTED
    } catch (_: Throwable) {
        false
    }

    /**
     * Wireless debugging can be paired on the device itself from Android 11.
     * Below that Shizuku needs a computer, which is a cost this product does
     * not ask a phone user to pay.
     */
    fun isSupported(): Boolean = Build.VERSION.SDK_INT >= Build.VERSION_CODES.R

    fun requestPermission(): Boolean {
        if (!isRunning()) {
            lastFailure = "shizuku is not running"
            return false
        }
        return try {
            Shizuku.requestPermission(PERMISSION_REQUEST)
            true
        } catch (e: Throwable) {
            Log.w(TAG, "requesting the shizuku permission failed", e)
            lastFailure = e.javaClass.simpleName
            false
        }
    }

    private const val PERMISSION_REQUEST = 4919

    /**
     * `Settings.Secure.CLIPBOARD_SHOW_ACCESS_NOTIFICATIONS`.
     *
     * Written through the Shizuku user service because an ordinary app may not
     * write `Settings.Secure`. Never called without an acknowledgement — the
     * gate is in `capture::model::authorise_toast`, on the Rust side, where it
     * is tested.
     */
    fun setToastSuppressed(suppressed: Boolean, completion: (Boolean) -> Unit) {
        ShizukuSettings.setClipboardAccessNotifications(suppressed) { changed ->
            if (!changed) lastFailure = "clipboard notice setting was refused"
            ClipboardNoticeSetting.invalidate()
            completion(changed)
        }
    }

    fun isToastSuppressed(context: android.content.Context): Boolean =
        ClipboardNoticeSetting.suppressed(context)
}
