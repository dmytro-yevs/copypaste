package com.copypaste.app

import android.app.Activity
import android.os.Bundle

/**
 * Transparent focus activity for tiles, the widget and the keyboard panel:
 * clipboard reads that need window focus, device authentication and short
 * forms. It is not exported; every entry point is an explicit intent.
 */
class QuickAccessActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        finish()
    }
}
