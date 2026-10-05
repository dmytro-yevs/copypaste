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

class ClipboardFloatingActivity : Activity() {
    private lateinit var windowManager: WindowManager
    private lateinit var floatingView: View
    private var attached = false
    private var handled = false
    private lateinit var focusListener: ViewTreeObserver.OnWindowFocusChangeListener
    private val main = Handler(Looper.getMainLooper())
    private val failsafe = Runnable(::finishCapture)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        shrinkActivityWindow()
        windowManager = getSystemService(Context.WINDOW_SERVICE) as WindowManager
        focusListener = ViewTreeObserver.OnWindowFocusChangeListener { hasFocus ->
            if (!hasFocus || handled) return@OnWindowFocusChangeListener
            handled = true
            floatingView.viewTreeObserver.removeOnWindowFocusChangeListener(focusListener)
            try {
                AndroidClipboardReader.captureBackground(this, intent.getLongExtra("capture-host", 0L))
            } finally {
                finishCapture()
            }
        }
        main.postDelayed(failsafe, captureTimeoutMs)
        try {
            createFloatingView()
            focusFloatingView()
        } catch (_: RuntimeException) {
            finishCapture()
        }
    }

    private fun shrinkActivityWindow() {
        val params = window.attributes
        params.width = 1
        params.height = 1
        params.gravity = Gravity.TOP or Gravity.START
        params.flags = params.flags or
            WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL or
            WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or
            WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS
        window.attributes = params
    }

    private fun createFloatingView() {
        floatingView = View(this)
        floatingView.viewTreeObserver.addOnWindowFocusChangeListener(focusListener)
        val params = WindowManager.LayoutParams(
            1,
            1,
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL or
                WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or
                WindowManager.LayoutParams.FLAG_WATCH_OUTSIDE_TOUCH,
            PixelFormat.TRANSLUCENT,
        ).apply { gravity = Gravity.TOP or Gravity.START }
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
            runCatching { floatingView.viewTreeObserver.removeOnWindowFocusChangeListener(focusListener) }
            runCatching { windowManager.removeViewImmediate(floatingView) }
            attached = false
        }
        if (!isFinishing) finish()
    }

    override fun onDestroy() {
        finishCapture()
        super.onDestroy()
    }

    companion object {
        private const val captureTimeoutMs = 400L

        fun intent(context: Context, hostId: Long): Intent =
            Intent(context, ClipboardFloatingActivity::class.java).apply {
                putExtra("capture-host", hostId)
                flags = Intent.FLAG_ACTIVITY_NEW_TASK or
                    Intent.FLAG_ACTIVITY_EXCLUDE_FROM_RECENTS or
                    Intent.FLAG_ACTIVITY_NO_ANIMATION
            }
    }
}
