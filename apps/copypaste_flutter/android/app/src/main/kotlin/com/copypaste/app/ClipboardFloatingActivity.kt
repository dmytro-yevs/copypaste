package com.copypaste.app

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.Gravity
import android.view.View
import android.view.WindowManager

class ClipboardFloatingActivity : Activity() {
    private var handled = false
    private val main = Handler(Looper.getMainLooper())
    private val failsafe = Runnable(::finishCapture)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        shrinkActivityWindow()
        setContentView(View(this))
        main.postDelayed(failsafe, captureTimeoutMs)
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (!hasFocus || handled) return
        handled = true
        // Finish only after Android has returned from its focus dispatch. Removing
        // a focused overlay synchronously crashes ViewRootImpl on some OEMs.
        main.post {
            if (isFinishing || isDestroyed) return@post
            try {
                AndroidClipboardReader.captureBackground(this, intent.getLongExtra("capture-host", 0L))
            } finally {
                finishCapture()
            }
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

    private fun finishCapture() {
        main.removeCallbacksAndMessages(null)
        if (!isFinishing) finish()
    }

    override fun onDestroy() {
        main.removeCallbacksAndMessages(null)
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
