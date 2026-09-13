package com.copypaste.app

import android.view.ViewGroup
import android.webkit.WebView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import java.util.WeakHashMap

internal object WebViewImeInsets {
  private val baseBottomMargins = WeakHashMap<WebView, Int>()

  fun install(
    webView: WebView,
    afterApply: (WebView, WindowInsetsCompat) -> Unit = { _, _ -> },
  ) {
    ViewCompat.setOnApplyWindowInsetsListener(webView) { view, insets ->
      val host = view as? WebView ?: return@setOnApplyWindowInsetsListener insets
      applyBottomMargin(host, insets)
      afterApply(host, insets)
      insets
    }
    ViewCompat.requestApplyInsets(webView)
  }

  private fun applyBottomMargin(webView: WebView, insets: WindowInsetsCompat) {
    val layoutParams = webView.layoutParams as? ViewGroup.MarginLayoutParams ?: return
    val baseBottomMargin = baseBottomMargins.getOrPut(webView) { layoutParams.bottomMargin }
    // System bars stay in CSS inset tokens. Shrinking the WebView for them
    // double-counts the dock and clips history tap targets.
    val visibleImeBottomInset = if (insets.isVisible(WindowInsetsCompat.Type.ime())) {
      insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
    } else {
      0
    }
    val desiredBottomMargin = baseBottomMargin + visibleImeBottomInset
    if (layoutParams.bottomMargin != desiredBottomMargin) {
      layoutParams.bottomMargin = desiredBottomMargin
      webView.layoutParams = layoutParams
    }
  }
}
