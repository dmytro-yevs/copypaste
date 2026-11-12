package com.copypaste.app

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import com.copypaste.app.widget.WidgetIntents
import com.copypaste.app.widget.WidgetTapHandler

/**
 * Transparent focus activity for tiles, the widget and the keyboard panel:
 * clipboard reads that need window focus, device authentication and short
 * forms. It is not exported; every entry point is an explicit intent.
 */
class QuickAccessActivity : Activity() {
    private var widgetTap: WidgetTapHandler? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val intent = intent
        if (savedInstanceState == null && intent?.action == WidgetIntents.ACTION_TAP) {
            widgetTap = WidgetTapHandler(this).also { it.start(intent) }
        } else {
            finish()
        }
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        if (widgetTap?.onActivityResult(requestCode, resultCode) == true) return
        super.onActivityResult(requestCode, resultCode, data)
    }

    override fun onDestroy() {
        widgetTap?.dispose()
        widgetTap = null
        super.onDestroy()
    }
}
