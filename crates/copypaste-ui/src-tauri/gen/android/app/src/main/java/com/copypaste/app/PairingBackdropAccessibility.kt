package com.copypaste.app

import android.graphics.Rect
import android.os.Build
import android.os.Bundle
import android.view.View
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityNodeProvider
import android.webkit.WebView

internal class PairingBackdropAccessibility(
  private val webView: WebView,
  private val label: CharSequence,
) {
  private var source: AccessibilityNodeProvider? = null
  private var wrapper: AccessibilityNodeProvider? = null

  fun wrap(provider: AccessibilityNodeProvider?): AccessibilityNodeProvider? {
    if (provider == null) {
      source = null
      wrapper = null
      return null
    }
    if (provider !== source) {
      source = provider
      wrapper = BackdropProvider(provider, webView, label)
    }
    return wrapper
  }

  private class BackdropProvider(
    private val source: AccessibilityNodeProvider,
    private val webView: WebView,
    private val label: CharSequence,
  ) : AccessibilityNodeProvider() {
    override fun createAccessibilityNodeInfo(virtualViewId: Int): AccessibilityNodeInfo? =
      source.createAccessibilityNodeInfo(virtualViewId)?.also(::labelBackdrop)

    override fun findAccessibilityNodeInfosByText(
      text: String,
      virtualViewId: Int,
    ): List<AccessibilityNodeInfo>? =
      source.findAccessibilityNodeInfosByText(text, virtualViewId)?.onEach(::labelBackdrop)

    override fun findFocus(focus: Int): AccessibilityNodeInfo? =
      source.findFocus(focus)?.also(::labelBackdrop)

    override fun performAction(virtualViewId: Int, action: Int, arguments: Bundle?): Boolean =
      source.performAction(virtualViewId, action, arguments)

    override fun addExtraDataToAccessibilityNodeInfo(
      virtualViewId: Int,
      info: AccessibilityNodeInfo,
      extraDataKey: String,
      arguments: Bundle?,
    ) {
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
        source.addExtraDataToAccessibilityNodeInfo(
          virtualViewId,
          info,
          extraDataKey,
          arguments,
        )
      }
    }

    private fun labelBackdrop(info: AccessibilityNodeInfo) {
      if (isPairingBackdrop(info)) info.contentDescription = label
    }

    // API 33 exposes Radix's body dismissal as an unnamed clickable virtual node.
    // The transient DOM id prevents this repair from naming any other full-screen dialog.
    private fun isPairingBackdrop(info: AccessibilityNodeInfo): Boolean {
      if (info.viewIdResourceName != PAIRING_BODY_ACCESSIBILITY_MARKER) return false
      if (!info.isClickable || info.childCount < 2) return false
      if (info.className?.toString() != View::class.java.name) return false
      if (!info.text.isNullOrBlank() || !info.contentDescription.isNullOrBlank()) return false
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && !info.hintText.isNullOrBlank()) return false

      val nodeBounds = Rect()
      info.getBoundsInScreen(nodeBounds)
      val location = IntArray(2)
      webView.getLocationOnScreen(location)
      return nodeBounds == Rect(
        location[0],
        location[1],
        location[0] + webView.width,
        location[1] + webView.height,
      )
    }
  }

  private companion object {
    const val PAIRING_BODY_ACCESSIBILITY_MARKER = "copypaste-pairing-dialog-open"
  }
}
