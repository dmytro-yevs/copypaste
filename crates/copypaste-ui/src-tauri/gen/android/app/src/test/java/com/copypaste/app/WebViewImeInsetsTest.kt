package com.copypaste.app

import android.content.Context
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import android.widget.FrameLayout
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class WebViewImeInsetsTest {
  @Test
  fun imeInsetsResizeAndRestoreTheActualWebViewWithoutChangingSystemBarLayout() {
    val root = HostLayout(RuntimeEnvironment.getApplication())
    val webView = LayoutWebView(RuntimeEnvironment.getApplication())
    root.addView(
      webView,
      FrameLayout.LayoutParams(
        ViewGroup.LayoutParams.MATCH_PARENT,
        ViewGroup.LayoutParams.MATCH_PARENT,
      ).apply { bottomMargin = ORIGINAL_BOTTOM_MARGIN },
    )
    layout(root)
    WebViewImeInsets.install(webView)

    dispatchInsets(webView, imeBottom = 0, systemBarBottom = SYSTEM_BAR_BOTTOM)
    layout(root)
    assertEquals(ORIGINAL_BOTTOM_MARGIN, bottomMarginOf(webView))
    assertEquals(ROOT_HEIGHT - ORIGINAL_BOTTOM_MARGIN, webView.measuredHeight)

    dispatchInsets(webView, imeBottom = IME_BOTTOM, systemBarBottom = SYSTEM_BAR_BOTTOM)
    layout(root)
    assertEquals(ORIGINAL_BOTTOM_MARGIN + IME_BOTTOM, bottomMarginOf(webView))
    assertEquals(ROOT_HEIGHT - ORIGINAL_BOTTOM_MARGIN - IME_BOTTOM, webView.measuredHeight)

    dispatchInsets(webView, imeBottom = IME_BOTTOM, systemBarBottom = SYSTEM_BAR_BOTTOM)
    assertFalse(webView.isLayoutRequested)
    assertEquals(ORIGINAL_BOTTOM_MARGIN + IME_BOTTOM, bottomMarginOf(webView))

    dispatchInsets(webView, imeBottom = 0, systemBarBottom = SYSTEM_BAR_BOTTOM)
    layout(root)
    assertEquals(ORIGINAL_BOTTOM_MARGIN, bottomMarginOf(webView))
    assertEquals(ROOT_HEIGHT - ORIGINAL_BOTTOM_MARGIN, webView.measuredHeight)
  }

  private fun dispatchInsets(webView: WebView, imeBottom: Int, systemBarBottom: Int) {
    val imeVisible = imeBottom > 0
    val insets = WindowInsetsCompat.Builder()
      .setInsets(
        WindowInsetsCompat.Type.systemBars(),
        Insets.of(0, 0, 0, systemBarBottom),
      )
      .setInsets(WindowInsetsCompat.Type.ime(), Insets.of(0, 0, 0, imeBottom))
      .setVisible(WindowInsetsCompat.Type.ime(), imeVisible)
      .build()

    ViewCompat.dispatchApplyWindowInsets(webView, insets)
  }

  private fun bottomMarginOf(webView: WebView): Int =
    (webView.layoutParams as ViewGroup.MarginLayoutParams).bottomMargin

  private fun layout(view: View) {
    val width = View.MeasureSpec.makeMeasureSpec(ROOT_WIDTH, View.MeasureSpec.EXACTLY)
    val height = View.MeasureSpec.makeMeasureSpec(ROOT_HEIGHT, View.MeasureSpec.EXACTLY)
    view.forceLayout()
    view.measure(width, height)
    view.layout(0, 0, ROOT_WIDTH, ROOT_HEIGHT)
  }

  private class LayoutWebView(context: android.content.Context) : WebView(context) {
    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
      setMeasuredDimension(
        View.MeasureSpec.getSize(widthMeasureSpec),
        View.MeasureSpec.getSize(heightMeasureSpec),
      )
    }
  }

  private class HostLayout(context: Context) : FrameLayout(context) {
    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
      val width = View.MeasureSpec.getSize(widthMeasureSpec)
      val height = View.MeasureSpec.getSize(heightMeasureSpec)
      setMeasuredDimension(width, height)

      getChildAt(0)?.let { child ->
        val margins = child.layoutParams as ViewGroup.MarginLayoutParams
        child.measure(
          View.MeasureSpec.makeMeasureSpec(
            width - margins.leftMargin - margins.rightMargin,
            View.MeasureSpec.EXACTLY,
          ),
          View.MeasureSpec.makeMeasureSpec(
            height - margins.topMargin - margins.bottomMargin,
            View.MeasureSpec.EXACTLY,
          ),
        )
      }
    }

    override fun onLayout(changed: Boolean, left: Int, top: Int, right: Int, bottom: Int) {
      getChildAt(0)?.let { child ->
        val margins = child.layoutParams as ViewGroup.MarginLayoutParams
        child.layout(
          margins.leftMargin,
          margins.topMargin,
          width - margins.rightMargin,
          height - margins.bottomMargin,
        )
      }
    }
  }

  private companion object {
    const val ROOT_WIDTH = 320
    const val ROOT_HEIGHT = 640
    const val ORIGINAL_BOTTOM_MARGIN = 12
    const val SYSTEM_BAR_BOTTOM = 24
    const val IME_BOTTOM = 240
  }
}
