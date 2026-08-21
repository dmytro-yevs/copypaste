package com.copypaste.app

import android.app.Activity
import android.content.Context
import android.os.Bundle

class ClipboardCaptureActivity : Activity() {
    private var handled = false

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (!hasFocus || handled) return

        handled = true
        val text = clipboardText(this)
        if (!text.isNullOrBlank()) queueClip(text, CaptureSource.TILE)
        finish()
    }
}
