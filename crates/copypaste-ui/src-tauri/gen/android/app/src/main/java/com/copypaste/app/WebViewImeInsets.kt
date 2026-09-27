package com.copypaste.app

import android.view.ViewGroup
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsAnimationCompat
import androidx.core.view.WindowInsetsCompat
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import java.util.WeakHashMap
import java.util.concurrent.atomic.AtomicReference
import kotlin.math.roundToInt

internal object WebViewImeInsets {
  private const val BRIDGE_NAME = "__copypasteSystemBarInsets"
  private const val APPLY_FUNCTION = "__copypasteApplySystemBarInsets"
  private val states = WeakHashMap<WebView, State>()

  fun install(webView: WebView) {
    val state = states.getOrPut(webView) {
      State(baseBottomMargin = bottomMarginOf(webView))
    }
    if (!state.bootstrapInstalled) {
      installBootstrap(webView, state)
      state.bootstrapInstalled = true
    }

    fun apply(host: WebView, insets: WindowInsetsCompat) {
      applyInsets(host, state, insets)
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

  fun replayCss(webView: WebView) {
    states[webView]?.latestCss?.get()?.let { publishCss(webView, it) }
  }

  private fun installBootstrap(webView: WebView, state: State) {
    // This bridge only returns current CSS inset values. It has no side
    // effects and is installed before Wry starts navigation in MainActivity.
    webView.addJavascriptInterface(
      InsetBridge(state.latestCss) {
        webView.post { replayCssAtDocumentReady(webView, state) }
      },
      BRIDGE_NAME,
    )
    if (WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) {
      WebViewCompat.addDocumentStartJavaScript(webView, documentStartScript(), setOf("*"))
    }
  }

  private fun replayCssAtDocumentReady(webView: WebView, state: State) {
    ViewCompat.getRootWindowInsets(webView)?.let { insets ->
      applyInsets(webView, state, insets)
    } ?: publishCss(webView, state.latestCss.get())
  }

  private fun applyBottomMargin(
    webView: WebView,
    baseBottomMargin: Int,
    insets: WindowInsetsCompat,
  ) {
    val layoutParams = webView.layoutParams as? ViewGroup.MarginLayoutParams ?: return
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

  private fun applyInsets(webView: WebView, state: State, insets: WindowInsetsCompat) {
    applyBottomMargin(webView, state.baseBottomMargin, insets)
    val css = InsetsCss.from(webView, insets)
    state.latestCss.set(css)
    publishCss(webView, css)
  }

  private fun bottomMarginOf(webView: WebView): Int =
    (webView.layoutParams as? ViewGroup.MarginLayoutParams)?.bottomMargin ?: 0

  private fun publishCss(webView: WebView, css: InsetsCss) {
    webView.evaluateJavascript(
      """
        (function (insets) {
          var apply = window.$APPLY_FUNCTION;
          if (apply) { apply(insets); return; }
          var root = document.documentElement;
          if (!root) return;
          root.style.setProperty('--inset-top', insets.top + 'px');
          root.style.setProperty('--inset-right', insets.right + 'px');
          root.style.setProperty('--inset-bottom', insets.bottom + 'px');
          root.style.setProperty('--inset-left', insets.left + 'px');
          if (insets.ime > 0) root.setAttribute('data-ime', '');
          else root.removeAttribute('data-ime');
        })(${css.asJson()});
      """.trimIndent(),
      null,
    )
  }

  internal fun documentStartScript(): String = """
    (function () {
      function apply(insets) {
        var root = document.documentElement;
        if (!root || !insets) return;
        root.style.setProperty('--inset-top', insets.top + 'px');
        root.style.setProperty('--inset-right', insets.right + 'px');
        root.style.setProperty('--inset-bottom', insets.bottom + 'px');
        root.style.setProperty('--inset-left', insets.left + 'px');
        if (insets.ime > 0) root.setAttribute('data-ime', '');
        else root.removeAttribute('data-ime');
      }
      function replay() {
        try { apply(JSON.parse(window.$BRIDGE_NAME.latest())); } catch (_) {}
      }
      window.$APPLY_FUNCTION = apply;
      replay();
      if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', replay, { once: true });
      }
    })();
  """.trimIndent()

  internal data class InsetsCss(
    val top: Int = 0,
    val right: Int = 0,
    val bottom: Int = 0,
    val left: Int = 0,
    val ime: Int = 0,
  ) {
    fun asJson(): String =
      "{\"top\":$top,\"right\":$right,\"bottom\":$bottom,\"left\":$left,\"ime\":$ime}"

    companion object {
      fun from(view: WebView, insets: WindowInsetsCompat): InsetsCss {
        val bars = insets.getInsets(
          WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout(),
        )
        val density = view.resources.displayMetrics.density.takeIf { it > 0f } ?: 1f
        val imeBottom = if (insets.isVisible(WindowInsetsCompat.Type.ime())) {
          cssPx(insets.getInsets(WindowInsetsCompat.Type.ime()).bottom, density)
        } else {
          0
        }
        // API 36 can report a visible IME window with a tiny inset while the
        // keyboard is not up. Only treat a real keyboard height as IME-open.
        return InsetsCss(
          top = cssPx(bars.top, density),
          right = cssPx(bars.right, density),
          bottom = cssPx(bars.bottom, density),
          left = cssPx(bars.left, density),
          ime = if (imeBottom >= 80) imeBottom else 0,
        )
      }

      private fun cssPx(pixels: Int, density: Float): Int =
        (pixels.toFloat() / density).roundToInt().coerceAtLeast(0)
    }
  }

  private class State(baseBottomMargin: Int) {
    val baseBottomMargin = baseBottomMargin
    val latestCss = AtomicReference(InsetsCss())
    var bootstrapInstalled = false
  }

  internal class InsetBridge(
    private val latestCss: AtomicReference<InsetsCss>,
    private val onDocumentReady: () -> Unit,
  ) {
    @JavascriptInterface
    fun latest(): String = latestCss.get().asJson()

    @JavascriptInterface
    fun ready() = onDocumentReady()
  }
}
