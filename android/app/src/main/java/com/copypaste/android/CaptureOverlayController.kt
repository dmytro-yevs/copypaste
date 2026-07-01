package com.copypaste.android

import android.app.Service
import android.graphics.PixelFormat
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.View
import android.view.WindowManager

/**
 * CopyPaste-vp63.32: the capture-overlay token trick, extracted VERBATIM from
 * [ClipboardService]'s instance methods ([addCaptureOverlay]/[removeCaptureOverlay])
 * and companion suppress/restore protocol.
 *
 * ## Background clipboard access (Android 10+)
 * `ClipboardManager.getPrimaryClip()` is blocked from any non-foreground,
 * non-IME context on API 29+. Adding a 1x1 invisible TYPE_APPLICATION_OVERLAY
 * window grants this process a WindowManager focus token, lifting the
 * restriction on Android 10+.
 *
 * ## Suppress/restore protocol (CopyPaste-xxi2 / CopyPaste-44rq.1)
 * [ClipboardFloatingActivity] needs exclusive window focus to read the
 * clipboard itself; a second overlay from this controller would conflict.
 * Protocol:
 *   1. ClipboardFloatingActivity calls [suppressCaptureOverlay] (removes this overlay).
 *   2. ClipboardFloatingActivity adds its own overlay and requests focus.
 *   3. After getPrimaryClip() succeeds/fails, ClipboardFloatingActivity calls
 *      [restoreCaptureOverlay] (re-adds this overlay).
 *
 * One instance is owned by the running [ClipboardService]; the companion
 * object holds a [java.lang.ref.WeakReference] to it (registered in
 * [ClipboardService.onCreate], cleared in [ClipboardService.onDestroy]) so the
 * static suppress/restore helpers can reach it without leaking the service.
 *
 * @param service the owning foreground service — used as [android.content.Context]
 *   for WindowManager/canDrawOverlays calls. Must not outlive the service; the
 *   companion only ever holds a weak reference.
 */
class CaptureOverlayController(private val service: Service) {

    /**
     * The 1x1 px invisible overlay view that gives this process a WindowManager
     * token, lifting the Android 10+ clipboard restriction so
     * getPrimaryClip() returns non-null from background.
     *
     * Non-null only when the overlay has been successfully added.
     * Guarded by [android.provider.Settings.canDrawOverlays] before the add call.
     */
    private var captureOverlayView: View? = null

    /**
     * Add a 1x1 px invisible overlay window so this process holds a
     * WindowManager token. On Android 10+ (API 29+) this token counts as
     * "focused" and lifts the clipboard restriction that blocks
     * getPrimaryClip() from background — the ClipCascade trick.
     *
     * Idempotent: does nothing if the overlay is already present.
     * Guarded by Settings.canDrawOverlays — on devices without the
     * SYSTEM_ALERT_WINDOW permission the call is a no-op and the existing
     * AccessibilityService path continues to be the background capture mechanism.
     *
     * Must be called from the main thread (WindowManager.addView requirement).
     * Only TYPE_APPLICATION_OVERLAY is legal for background services on API 26+.
     *
     * FLAG_NOT_TOUCHABLE | FLAG_NOT_FOCUSABLE: the overlay is completely
     * invisible and input-transparent — it cannot steal focus or touches from
     * the user. Its sole purpose is giving the process a window token.
     */
    fun add() {
        if (captureOverlayView != null) return // already present — idempotent

        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M) return // canDrawOverlays needs API 23
        if (!android.provider.Settings.canDrawOverlays(service)) {
            Log.d(TAG, "addCaptureOverlay: SYSTEM_ALERT_WINDOW not granted — skipping overlay")
            return
        }

        val wm = service.getSystemService(android.content.Context.WINDOW_SERVICE) as? WindowManager ?: run {
            Log.w(TAG, "addCaptureOverlay: WindowManager unavailable")
            return
        }

        val params = WindowManager.LayoutParams(
            /* width  */ 1,
            /* height */ 1,
            /* type   */ WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            /* flags  */ WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE,
            /* format */ PixelFormat.TRANSLUCENT
        ).apply {
            alpha = 0f // fully transparent — invisible to the user
        }

        val view = View(service)
        try {
            wm.addView(view, params)
            captureOverlayView = view
            Log.i(TAG, "addCaptureOverlay: invisible overlay added — background clipboard reads enabled")
        } catch (e: Exception) {
            // addView can throw if the permission was revoked between the
            // canDrawOverlays check and the addView call, or on some OEM ROMs
            // that return false from canDrawOverlays at add-time. Non-fatal —
            // fall back to the AccessibilityService capture path.
            Log.w(TAG, "addCaptureOverlay: addView failed (${e.javaClass.simpleName}: ${e.message})")
        }
    }

    /**
     * Remove the capture overlay if it was added. Idempotent.
     * Safe to call from onDestroy even if [add] was never called or failed.
     */
    fun remove() {
        val view = captureOverlayView ?: return
        captureOverlayView = null
        val wm = service.getSystemService(android.content.Context.WINDOW_SERVICE) as? WindowManager ?: return
        try {
            wm.removeView(view)
            Log.i(TAG, "removeCaptureOverlay: overlay removed")
        } catch (e: Exception) {
            // removeView can throw if the view was already detached (e.g. the
            // WindowManager died or the permission was revoked). Non-fatal.
            Log.w(TAG, "removeCaptureOverlay: removeView failed (${e.javaClass.simpleName}: ${e.message})")
        }
    }

    companion object {
        private const val TAG = "ClipboardService"

        /**
         * CopyPaste-xxi2: overlay suppression flag — set by [ClipboardFloatingActivity]
         * before it requests window focus so the service overlay is removed and the two-
         * overlay focus-token conflict is eliminated.
         *
         * The flag is @Volatile because it is written on the main thread (from the Activity)
         * and read on the service main thread.
         */
        @Volatile
        private var captureOverlaySuppressed: Boolean = false

        /**
         * Weak reference to the live [CaptureOverlayController] instance, used by the
         * static [suppressCaptureOverlay] / [restoreCaptureOverlay] helpers so they can
         * call [remove] / [add] without leaking the service. Registered in
         * [ClipboardService.onCreate], cleared in [ClipboardService.onDestroy].
         */
        @Volatile
        private var instance: java.lang.ref.WeakReference<CaptureOverlayController>? = null

        /** Register the live controller instance. Called from [ClipboardService.onCreate]. */
        fun register(controller: CaptureOverlayController) {
            instance = java.lang.ref.WeakReference(controller)
        }

        /** Clear the live controller instance. Called from [ClipboardService.onDestroy]. */
        fun clear() {
            instance = null
        }

        /**
         * CopyPaste-xxi2 / bd CopyPaste-44rq.1: remove the ClipboardService capture overlay
         * temporarily so [ClipboardFloatingActivity] can gain exclusive window focus without
         * a two-overlay conflict. Called by [ClipboardFloatingActivity] before adding its own overlay.
         *
         * Delegates to [remove] on the live service's overlay controller via [instance].
         * WindowManager.removeView must run on the main thread; posted there if not already.
         * No-op when the service is not running or no overlay was added.
         */
        fun suppressCaptureOverlay() {
            captureOverlaySuppressed = true
            Handler(Looper.getMainLooper()).post {
                val controller = instance?.get()
                if (controller != null) {
                    controller.remove()
                    Log.d(TAG, "suppressCaptureOverlay: overlay removed for FloatingActivity")
                } else {
                    Log.d(TAG, "suppressCaptureOverlay: service not running — no overlay to remove")
                }
            }
        }

        /**
         * CopyPaste-xxi2: signal that [ClipboardFloatingActivity] has released its overlay,
         * allowing [ClipboardService] to restore its capture overlay if it was suppressed.
         * Called by [ClipboardFloatingActivity] in its cleanup path.
         */
        fun restoreCaptureOverlay() {
            captureOverlaySuppressed = false
            Handler(Looper.getMainLooper()).post {
                val controller = instance?.get()
                if (controller != null) {
                    controller.add()
                    Log.d(TAG, "restoreCaptureOverlay: overlay re-added after FloatingActivity released")
                } else {
                    Log.d(TAG, "restoreCaptureOverlay: service not running — nothing to restore")
                }
            }
        }
    }
}
