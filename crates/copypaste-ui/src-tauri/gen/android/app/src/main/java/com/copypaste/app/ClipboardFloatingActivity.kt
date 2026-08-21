package com.copypaste.app

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.graphics.PixelFormat
import android.os.Bundle
import android.view.View
import android.view.ViewTreeObserver
import android.view.WindowManager

class ClipboardFloatingActivity : Activity() {
    private lateinit var windowManager: WindowManager
    private lateinit var floatingView: View
    private var attached = false
    private var handled = false
    private lateinit var layoutListener: ViewTreeObserver.OnGlobalLayoutListener

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        windowManager = getSystemService(Context.WINDOW_SERVICE) as WindowManager
        createFloatingView()
        focusFloatingView()
        layoutListener = ViewTreeObserver.OnGlobalLayoutListener {
            try {
                if (handled) return@OnGlobalLayoutListener
                handled = true
                floatingView.viewTreeObserver.removeOnGlobalLayoutListener(layoutListener)
                clipboardText(this)?.let { text ->
                    ClipQueue.offer(text, CaptureSource.BACKGROUND)
                    if (!ClipQueue.rustIsUp) {
                        startActivity(
                            Intent(this, MainActivity::class.java)
                                .addFlags(
                                    Intent.FLAG_ACTIVITY_NEW_TASK or
                                        Intent.FLAG_ACTIVITY_CLEAR_TOP,
                                ),
                        )
                    }
                }
            } finally {
                unfocusFloatingView()
                removeFloatingView()
            }
        }
        floatingView.viewTreeObserver.addOnGlobalLayoutListener(layoutListener)
    }

    private fun createFloatingView() {
        floatingView = View(this)
        val params = WindowManager.LayoutParams(
            1,
            1,
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                WindowManager.LayoutParams.FLAG_WATCH_OUTSIDE_TOUCH,
            PixelFormat.TRANSLUCENT,
        ).apply {
            x = 0
            y = 0
        }
        windowManager.addView(floatingView, params)
        attached = true
    }

    private fun focusFloatingView() {
        if (!attached) return
        val params = floatingView.layoutParams as WindowManager.LayoutParams
        params.flags = params.flags and WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE.inv()
        windowManager.updateViewLayout(floatingView, params)
    }

    private fun unfocusFloatingView() {
        if (!attached) return
        val params = floatingView.layoutParams as WindowManager.LayoutParams
        params.flags = params.flags or WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE
        windowManager.updateViewLayout(floatingView, params)
    }

    private fun removeFloatingView() {
        if (attached) {
            try {
                floatingView.viewTreeObserver.removeOnGlobalLayoutListener(layoutListener)
            } catch (_: Exception) {
            }
            windowManager.removeViewImmediate(floatingView)
            attached = false
        }
    }

    override fun onDestroy() {
        removeFloatingView()
        super.onDestroy()
    }

    companion object {
        fun intent(context: Context): Intent =
            Intent(context, ClipboardFloatingActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_NEW_TASK or
                    Intent.FLAG_ACTIVITY_CLEAR_TASK or
                    Intent.FLAG_ACTIVITY_EXCLUDE_FROM_RECENTS
            }
    }
}
