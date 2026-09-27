package com.copypaste.app

import android.webkit.WebView
import androidx.appcompat.app.AppCompatActivity

abstract class TauriActivity : AppCompatActivity() {
  open val handleBackNavigation: Boolean = false
  open fun onWebViewCreate(webView: WebView) {}
}
