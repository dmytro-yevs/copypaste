package com.copypaste.app

import android.webkit.WebView
import androidx.appcompat.app.AppCompatActivity

abstract class TauriActivity : AppCompatActivity() {
  open fun onWebViewCreate(webView: WebView) {}
}
