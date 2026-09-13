package com.copypaste.app

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.graphics.PixelFormat
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.Gravity
import android.view.View
import android.view.ViewTreeObserver
import android.view.WindowManager

/**
 * Brief 1×1 overlay used only to become the focused UID so a background
 * clipboard read is legal. A focusable overlay without [FLAG_NOT_TOUCH_MODAL]
 * is modal and swallows every touch on the screen — that is what blocked the
 * other app after a copy.
 */
class ClipboardFloatingActivity : Activity() {
    private lateinit var windowManager: WindowManager
    private lateinit var floatingView: View
    private var attached = false
    private var handled = false
    private lateinit var layoutListener: ViewTreeObserver.OnGlobalLayoutListener
    private val main = Handler(Looper.getMainLooper())
    private val failsafe = Runnable { finishCapture() }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        shrinkActivityWindow()
        windowManager = getSystemService(Context.WINDOW_SERVICE) as WindowManager
        createFloatingView()
        focusFloatingView()
        layoutListener = ViewTreeObserver.OnGlobalLayoutListener {
            if (handled) return@OnGlobalLayoutListener
            handled = true
            floatingView.viewTreeObserver.removeOnGlobalLayoutListener(layoutListener)
            try {
                val read = clipboardRead(this, CaptureSource.BACKGROUND)
                read.text?.let { text ->
                    queueClip(
                        text,
                        CaptureSource.BACKGROUND,
                        read.sourceAppBundleId,
                        read.sourceAppName,
                    )
                }
            } finally {
                finishCapture()
            }
        }
        floatingView.viewTreeObserver.addOnGlobalLayoutListener(layoutListener)
        main.postDelayed(failsafe, CAPTURE_TIMEOUT_MS)
    }

    private fun shrinkActivityWindow() {
        val params = window.attributes
        params.width = 1
        params.height = 1
        params.x = 0
        params.y = 0
        params.gravity = Gravity.TOP or Gravity.START
        params.flags = params.flags or
            WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL or
            WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or
            WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS
        window.attributes = params
    }

    private fun createFloatingView() {
        floatingView = View(this)
        val params = WindowManager.LayoutParams(
            1,
            1,
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL or
                WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or
                WindowManager.LayoutParams.FLAG_WATCH_OUTSIDE_TOUCH,
            PixelFormat.TRANSLUCENT,
        ).apply {
            gravity = Gravity.TOP or Gravity.START
            x = 0
            y = 0
        }
        windowManager.addView(floatingView, params)
        attached = true
    }

    private fun focusFloatingView() {
        if (!attached) return
        val params = floatingView.layoutParams as WindowManager.LayoutParams
        params.flags = WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL or
            WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE
        windowManager.updateViewLayout(floatingView, params)
    }

    private fun finishCapture() {
        main.removeCallbacks(failsafe)
        if (attached) {
            try {
                floatingView.viewTreeObserver.removeOnGlobalLayoutListener(layoutListener)
            } catch (_: Exception) {
            }
            try {
                windowManager.removeViewImmediate(floatingView)
            } catch (_: Exception) {
            }
            attached = false
        }
        if (!isFinishing) finish()
    }

    override fun onDestroy() {
        finishCapture()
        super.onDestroy()
    }

    companion object {
        private const val CAPTURE_TIMEOUT_MS = 400L

        fun intent(context: Context): Intent =
            Intent(context, ClipboardFloatingActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_NEW_TASK or
                    Intent.FLAG_ACTIVITY_EXCLUDE_FROM_RECENTS or
                    Intent.FLAG_ACTIVITY_NO_ANIMATION
            }
    }
}
