package com.copypaste.app

import android.view.ViewGroup
import android.webkit.WebView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsAnimationCompat
import androidx.core.view.WindowInsetsCompat
import java.util.WeakHashMap

internal object WebViewImeInsets {
  private val baseBottomMargins = WeakHashMap<WebView, Int>()

  // Compact Library chrome plus one 77px row. A 640px emulator IME can be
  // ~390px; applying all of that leaves a 124px list whose row hit target
  // is no longer tappable.
  private const val MIN_WEBVIEW_HEIGHT_PX = 420

  fun install(
    webView: WebView,
    afterApply: (WebView, WindowInsetsCompat) -> Unit = { _, _ -> },
  ) {
    fun apply(host: WebView, insets: WindowInsetsCompat) {
      applyBottomMargin(host, insets)
      afterApply(host, insets)
    }
    ViewCompat.setOnApplyWindowInsetsListener(webView) { view, insets ->
      val host = view as? WebView ?: return@setOnApplyWindowInsetsListener insets
      apply(host, insets)
      insets
    }
    ViewCompat.setWindowInsetsAnimationCallback(
      webView,
      object : WindowInsetsAnimationCompat.Callback(
        WindowInsetsAnimationCompat.Callback.DISPATCH_MODE_CONTINUE_ON_SUBTREE,
      ) {
        override fun onProgress(
          insets: WindowInsetsCompat,
          runningAnimations: MutableList<WindowInsetsAnimationCompat>,
        ): WindowInsetsCompat {
          apply(webView, insets)
          return insets
        }

        override fun onEnd(animation: WindowInsetsAnimationCompat) {
          ViewCompat.getRootWindowInsets(webView)?.let { apply(webView, it) }
        }
      },
    )
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
    val hostHeight = (webView.parent as? ViewGroup)?.height?.takeIf { it > 0 }
      ?: webView.height
    val maxImeInset = if (hostHeight > 0) {
      (hostHeight - MIN_WEBVIEW_HEIGHT_PX).coerceAtLeast(0)
    } else {
      visibleImeBottomInset
    }
    val desiredBottomMargin = baseBottomMargin +
      visibleImeBottomInset.coerceAtMost(maxImeInset)
    if (layoutParams.bottomMargin != desiredBottomMargin) {
      layoutParams.bottomMargin = desiredBottomMargin
      webView.layoutParams = layoutParams
    }
  }
}
