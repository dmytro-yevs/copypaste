package com.copypaste.app

import android.content.Context
import android.os.Build
import android.os.IBinder
import android.os.Process
import android.util.Log
import rikka.shizuku.Shizuku
import rikka.shizuku.ShizukuBinderWrapper
import rikka.shizuku.SystemServiceHelper

/**
 * Shizuku is the rung-2 setup/settings and source-attribution bridge.
 *
 * The live clipboard reader is app-owned (`ClipCascadeCapture` +
 * `ClipboardFloatingActivity` + `CaptureService`); clipboard content never
 * crosses this bridge. Shizuku remains for:
 * - checking whether its server is available;
 * - requesting our one-time permission; and
 * - calling the user service that applies ClipCascade-style grants and the
 *   optional clipboard-notice setting; and
 * - asking the clipboard service for source-package metadata before a read.
 */
object ShizukuClipboard {
    private const val TAG = "CopyPasteShizuku"
    private const val SHELL_PACKAGE = "com.android.shell"
    private const val PER_USER_RANGE = 100_000

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
    internal fun sourcePackage(context: Context): String? {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) return null
        if (!isRunning() || !hasPermission()) return null
        return try {
            // Mirrors hidden UserHandle.getUserId; the public SDK exposes no numeric accessor.
            val userId = Process.myUid() / PER_USER_RANGE
            val rawBinder = SystemServiceHelper.getSystemService("clipboard")
            val binder: IBinder = ShizukuBinderWrapper(rawBinder)
            val stub = Class.forName("android.content.IClipboard\$Stub")
            val service = stub
                .getMethod("asInterface", IBinder::class.java)
                .invoke(null, binder)
                ?: return null
            val clipboard = Class.forName("android.content.IClipboard")
            val source = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                val deviceId = context.deviceId
                clipboard
                    .getMethod(
                        "getPrimaryClipSource",
                        String::class.java,
                        String::class.java,
                        Int::class.javaPrimitiveType,
                        Int::class.javaPrimitiveType,
                    )
                    .invoke(service, SHELL_PACKAGE, null, userId, deviceId)
            } else {
                clipboard
                    .getMethod(
                        "getPrimaryClipSource",
                        String::class.java,
                        Int::class.javaPrimitiveType,
                    )
                    .invoke(service, SHELL_PACKAGE, userId)
            }
            (source as? String)
                ?.trim()
                ?.takeIf { it.isNotEmpty() && it != SHELL_PACKAGE }
        } catch (error: Throwable) {
            lastFailure = error.javaClass.simpleName
            Log.d(TAG, "clipboard source attribution is unavailable")
            null
        }
    }

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
