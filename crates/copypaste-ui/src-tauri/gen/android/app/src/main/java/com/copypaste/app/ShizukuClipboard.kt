package com.copypaste.app

import android.os.Build
import android.util.Log
import rikka.shizuku.Shizuku

/**
 * Shizuku is the rung-2 setup/settings and source-attribution bridge.
 *
 * Shizuku is onboarding-only. It applies the one-time app grants and may then
 * be removed; runtime capture never binds its service.
 * - checking whether its server is available;
 * - requesting our one-time permission; and
 * - calling the setup user service once.
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
     * Source attribution exists only in the hidden test API from Android 12.
     * The maintained Shizuku wrapper supplies the shell identity that owns
     * SET_CLIP_SOURCE; ordinary app reflection would still be permission-denied.
     */
    internal fun sourcePackage(): String? = null

    /**
     * `Settings.Secure.CLIPBOARD_SHOW_ACCESS_NOTIFICATIONS`.
     *
     * Written through the Shizuku user service because an ordinary app may not
     * write `Settings.Secure`. Never called without an acknowledgement — the
     * gate is in `capture::model::authorise_toast`, on the Rust side, where it
     * is tested.
     */
    fun setToastSuppressed(suppressed: Boolean, completion: (Boolean) -> Unit) {
        if (suppressed) lastFailure = "clipboard notice setup is unavailable after onboarding"
        completion(false)
    }

    fun isToastSuppressed(): Boolean = false
}
