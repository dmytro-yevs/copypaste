package com.copypaste.app

import android.view.ViewGroup
import android.webkit.WebView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

internal object WebViewImeInsets {
  fun install(webView: WebView) {
    var originalBottomMargin: Int? = null

    ViewCompat.setOnApplyWindowInsetsListener(webView) { view, insets ->
      val layoutParams = view.layoutParams as? ViewGroup.MarginLayoutParams
      if (layoutParams != null) {
        val baseBottomMargin = originalBottomMargin ?: layoutParams.bottomMargin.also {
          originalBottomMargin = it
        }
        val imeBottomInset = if (insets.isVisible(WindowInsetsCompat.Type.ime())) {
          insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
        } else {
          0
        }
        val desiredBottomMargin = baseBottomMargin + imeBottomInset

        if (layoutParams.bottomMargin != desiredBottomMargin) {
          layoutParams.bottomMargin = desiredBottomMargin
          view.layoutParams = layoutParams
        }
      }
      insets
    }
    ViewCompat.requestApplyInsets(webView)
  }
}
