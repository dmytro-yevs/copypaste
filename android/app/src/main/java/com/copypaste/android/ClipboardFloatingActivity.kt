package com.copypaste.android

import android.app.Activity
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.PixelFormat
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.View
import android.view.ViewTreeObserver
import android.view.WindowManager
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch

/**
 * Transparent focusable overlay Activity that performs a background clipboard read.
 *
 * ## Why this Activity exists
 * On Android 10+ (API 29+), [ClipboardManager.getPrimaryClip] returns null from any
 * context that is not the foreground app, the default IME, or an enabled
 * AccessibilityService — even from a foreground service with [TYPE_APPLICATION_OVERLAY].
 * A non-focusable overlay never receives input focus, so the restriction is NOT lifted.
 *
 * The ClipCascade technique bypasses this:
 *   1. A daemon thread tails logcat for the OS-emitted clipboard-access denial line
 *      (which names our package). Detection fires from [LogcatCaptureService].
 *   2. THIS Activity is launched — transparent and floating, excluded from recents.
 *   3. In [onCreate] we add a TYPE_APPLICATION_OVERLAY view that starts NOT_FOCUSABLE,
 *      then immediately CLEARS that flag and calls updateViewLayout to request focus.
 *   4. We wait for the window-layout pass via [ViewTreeObserver.OnGlobalLayoutListener].
 *      ONLY inside that callback (after focus has been gained) do we call
 *      [ClipboardManager.getPrimaryClip] — by then the OS clipboard restriction is lifted
 *      because the overlay window is focused.
 *   5. The captured clip is routed through [ClipboardService.captureClip] /
 *      [ClipboardService.captureImageClip] / [ClipboardService.captureFileClip] — the
 *      SAME shared pipeline as the foreground service, so dedup, sensitive-detection,
 *      and sync all apply.
 *   6. We re-set FLAG_NOT_FOCUSABLE, remove the overlay view, and call [finish].
 *
 * The Activity is themed with [Theme.CopyPaste.FloatingOverlay] (defined in themes.xml):
 *   windowIsTranslucent=true, windowIsFloating=true, transparent windowBackground,
 *   backgroundDimEnabled=false. The user sees nothing — the Activity finishes in
 *   milliseconds.
 *
 * ## Safety guards
 * - [android.provider.Settings.canDrawOverlays] checked before addView; if not granted
 *   the Activity finishes immediately.
 * - Only launched on API 29+ (gated in [LogcatCaptureService]).
 * - All WindowManager calls wrapped in try/catch — a failed add or update never crashes.
 * - [finish] is called in every code path (success, guard failure, exception).
 *
 * ## Load-bearing detail
 * The [ClipboardManager.getPrimaryClip] call MUST be inside the
 * [ViewTreeObserver.OnGlobalLayoutListener] callback. Calling it synchronously after
 * [WindowManager.addView] or immediately after clearing FLAG_NOT_FOCUSABLE does NOT
 * work — the OS lifts the clipboard restriction only after the window-focus event,
 * which arrives with the next layout pass. Reading early returns null.
 */
class ClipboardFloatingActivity : Activity() {

    // SupervisorJob: a failing capture coroutine (bad image decode, etc.) does not
    // cancel sibling coroutines — the scope remains live until finish() is called.
    private val scope = CoroutineScope(Dispatchers.IO + SupervisorJob())

    private var overlayView: View? = null
    private var overlayParams: WindowManager.LayoutParams? = null
    private lateinit var wm: WindowManager
    private var layoutListener: ViewTreeObserver.OnGlobalLayoutListener? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Guard: overlay permission required. If revoked since the logcat trigger fired,
        // abort immediately rather than throwing in addView.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M &&
            !android.provider.Settings.canDrawOverlays(this)
        ) {
            Log.w(TAG, "canDrawOverlays = false — aborting focused clipboard read")
            finish()
            return
        }

        wm = getSystemService(Context.WINDOW_SERVICE) as WindowManager

        // Step 1: Add the overlay as NOT_FOCUSABLE first. The view is 1×1 px and
        // fully transparent — invisible to the user. FLAG_WATCH_OUTSIDE_TOUCH ensures
        // the Activity receives touch events outside its bounds (harmless but needed
        // by the ClipCascade pattern to hold focus correctly on some OEM ROMs).
        val params = WindowManager.LayoutParams(
            /* width  */ 1,
            /* height */ 1,
            /* type   */ WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            /* flags  */ WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                WindowManager.LayoutParams.FLAG_WATCH_OUTSIDE_TOUCH,
            /* format */ PixelFormat.TRANSLUCENT
        ).apply {
            alpha = 0f  // fully transparent
        }

        val view = View(this)
        try {
            wm.addView(view, params)
            overlayView = view
            overlayParams = params
        } catch (e: Exception) {
            Log.w(TAG, "addView failed: ${e.message} — aborting")
            finish()
            return
        }

        // Step 2: CLEAR FLAG_NOT_FOCUSABLE so the overlay window can gain input focus.
        // This is the load-bearing step: without clearing this flag, getPrimaryClip()
        // returns null even inside the layout listener because the OS never grants
        // clipboard access to a non-focusable window.
        params.flags = params.flags and WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE.inv()
        try {
            wm.updateViewLayout(view, params)
        } catch (e: Exception) {
            Log.w(TAG, "updateViewLayout (focus request) failed: ${e.message} — aborting")
            cleanupAndFinish()
            return
        }

        // Step 3: Register a ONE-SHOT layout listener. The clipboard read MUST happen
        // inside this callback — only after the layout/focus pass does the OS lift the
        // API-29+ clipboard restriction. Reading before this point returns null even
        // though focus was requested in step 2.
        val listener = ViewTreeObserver.OnGlobalLayoutListener { onFocusedLayout() }
        layoutListener = listener
        view.viewTreeObserver.addOnGlobalLayoutListener(listener)
    }

    /**
     * Called from [ViewTreeObserver.OnGlobalLayoutListener] after the window has
     * gained focus. This is the ONLY safe point to call [ClipboardManager.getPrimaryClip].
     */
    private fun onFocusedLayout() {
        // Remove the listener immediately — fire once only.
        val view = overlayView
        if (view != null) {
            val listener = layoutListener
            if (listener != null) {
                view.viewTreeObserver.removeOnGlobalLayoutListener(listener)
                layoutListener = null
            }
        }

        val cm = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val clip = cm.primaryClip

        if (clip == null) {
            Log.d(TAG, "getPrimaryClip returned null even with focused overlay — restriction not lifted on this ROM")
            cleanupAndFinish()
            return
        }

        Log.d(TAG, "getPrimaryClip succeeded via focused overlay — routing to capture pipeline")

        val settings = Settings(this)
        val repository = ClipboardRepository(this)
        val relayHttp = RelayClient(settings.relayUrl)
        val syncManager = SyncManager(relayHttp, settings.deviceId, token = "", settings = settings)

        // Detect MIME type and route to the appropriate shared capture function,
        // exactly mirroring the foreground ClipboardService clipListener dispatch.
        val imageMime = (0 until clip.description.mimeTypeCount)
            .map { clip.description.getMimeType(it) }
            .firstOrNull { it.startsWith("image/") }

        if (imageMime != null) {
            val uri = clip.getItemAt(0)?.uri
            if (uri != null) {
                scope.launch {
                    ClipboardService.captureImageClip(
                        this@ClipboardFloatingActivity, uri, imageMime,
                        settings, repository, syncManager
                    )
                }
            } else {
                Log.w(TAG, "Image clip has no URI — skipping")
            }
            cleanupAndFinish()
            return
        }

        // File branch: non-text, non-image URI = real file (PDF, ZIP, DOCX, etc.)
        val itemUri = clip.getItemAt(0)?.uri
        if (itemUri != null) {
            val mimeTypes = (0 until clip.description.mimeTypeCount)
                .map { clip.description.getMimeType(it) }
            val fileMime = mimeTypes.firstOrNull { mime ->
                mime != null && !mime.startsWith("text/") && !mime.startsWith("image/")
            }
            if (fileMime != null) {
                scope.launch {
                    ClipboardService.captureFileClip(
                        this@ClipboardFloatingActivity, itemUri, fileMime,
                        settings, repository, syncManager
                    )
                }
                cleanupAndFinish()
                return
            }
        }

        // Text branch (most common)
        val text = clip.getItemAt(0)?.text?.toString()
        if (!text.isNullOrBlank()) {
            scope.launch {
                ClipboardService.captureClip(
                    this@ClipboardFloatingActivity, text,
                    settings, repository, syncManager
                )
            }
        } else {
            Log.d(TAG, "Clip has no usable text/URI — skipping")
        }

        cleanupAndFinish()
    }

    /**
     * Restore FLAG_NOT_FOCUSABLE, remove the overlay view, cancel the scope, finish().
     * Safe to call multiple times — overlayView is nulled before the first removeView.
     */
    private fun cleanupAndFinish() {
        val view = overlayView ?: run {
            scope.cancel()
            finish()
            return
        }
        overlayView = null

        val params = overlayParams
        if (params != null) {
            // Re-set NOT_FOCUSABLE before removing so the transition is clean.
            params.flags = params.flags or WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE
            try { wm.updateViewLayout(view, params) } catch (_: Exception) { /* view may already be gone */ }
        }

        try {
            wm.removeView(view)
        } catch (e: Exception) {
            Log.d(TAG, "removeView (non-fatal): ${e.message}")
        }

        // Do NOT cancel scope here: launched capture coroutines must drain their
        // SharedPreferences writes before the process yields. The SupervisorJob and
        // Dispatchers.IO coroutines are typically fast (< 50 ms). The scope will be
        // GC'd once all children complete naturally.
        finish()
    }

    override fun onDestroy() {
        // Defensive cleanup in case cleanupAndFinish was not reached (e.g. system kill).
        val view = overlayView
        overlayView = null
        if (view != null) {
            try { wm.removeView(view) } catch (_: Exception) { }
        }
        scope.cancel()
        super.onDestroy()
    }

    companion object {
        private const val TAG = "ClipboardFloatingAct"

        /**
         * Launch the transparent focused overlay Activity from any context.
         *
         * Callers should gate this on:
         *   - [Build.VERSION.SDK_INT] >= [Build.VERSION_CODES.Q] (API 29+)
         *   - [android.provider.Settings.canDrawOverlays] == true
         *   - [LogcatCaptureService.hasReadLogsPermission] == true
         *
         * The Activity is completely transparent and excluded from the Recents list;
         * it finishes within milliseconds.
         */
        fun launch(context: Context) {
            val intent = Intent(context, ClipboardFloatingActivity::class.java).apply {
                addFlags(
                    Intent.FLAG_ACTIVITY_NEW_TASK or
                        Intent.FLAG_ACTIVITY_CLEAR_TASK or
                        Intent.FLAG_ACTIVITY_EXCLUDE_FROM_RECENTS
                )
            }
            try {
                context.startActivity(intent)
            } catch (e: Exception) {
                Log.w(TAG, "Failed to launch ClipboardFloatingActivity: ${e.message}")
            }
        }
    }
}
