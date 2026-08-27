package com.copypaste.app

import android.app.Activity
import android.content.Context
import android.graphics.Rect
import android.os.Bundle
import android.view.View
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityNodeProvider
import android.webkit.WebView
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotSame
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.android.controller.ActivityController
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33])
class PairingBackdropAccessibilityTest {
  private lateinit var activityController: ActivityController<Activity>
  private lateinit var host: FixtureHost
  private lateinit var provider: FixtureProvider

  @Before
  fun setUp() {
    activityController = Robolectric.buildActivity(Activity::class.java).setup()
    val activity = activityController.get()
    host = FixtureHost(activity)
    activity.setContentView(host)
    host.layout(0, 0, 320, 640)
    provider = FixtureProvider(host, "first")
    host.sourceProvider = provider
  }

  @After
  fun tearDown() {
    host.destroy()
    activityController.close()
  }

  @Test
  fun labelsOnlyExactPairingMarkerAndPreservesDescendants() {
    val backdrop = host.getAccessibilityNodeProvider()!!
      .createAccessibilityNodeInfo(BACKDROP_ID)!!
    val scanner = host.getAccessibilityNodeProvider()!!
      .createAccessibilityNodeInfo(SCANNER_ID)!!

    assertEquals("Dismiss pairing dialog", backdrop.contentDescription)
    assertEquals(PAIRING_MARKER, backdrop.viewIdResourceName)
    assertEquals(2, backdrop.childCount)
    assertTrue(backdrop.isClickable)
    assertTrue(backdrop.actionList.contains(AccessibilityNodeInfo.AccessibilityAction.ACTION_CLICK))
    assertEquals("Scan pairing code", scanner.text)
    assertNull(scanner.contentDescription)
  }

  @Test
  fun leavesGenericAndIncompleteTreesUnnamed() {
    val wrapped = host.getAccessibilityNodeProvider()!!
    val generic = wrapped.createAccessibilityNodeInfo(GENERIC_DIALOG_ID)!!
    val zeroChildren = wrapped.createAccessibilityNodeInfo(ZERO_CHILD_ID)!!
    val oneChild = wrapped.createAccessibilityNodeInfo(ONE_CHILD_ID)!!
    val partialBounds = wrapped.createAccessibilityNodeInfo(PARTIAL_BOUNDS_ID)!!

    assertNull(generic.contentDescription)
    assertEquals(2, generic.childCount)
    assertNull(zeroChildren.contentDescription)
    assertEquals(0, zeroChildren.childCount)
    assertNull(oneChild.contentDescription)
    assertEquals(1, oneChild.childCount)
    assertNull(partialBounds.contentDescription)
    assertEquals(2, partialBounds.childCount)
  }

  @Test
  fun blankOneChildTreeRemainsUnnamedAndHasNoScanner() {
    val blankProvider = FixtureProvider(host, "blank", includeScanner = false)
    host.sourceProvider = blankProvider
    val wrapped = host.getAccessibilityNodeProvider()!!
    val root = wrapped.createAccessibilityNodeInfo(BACKDROP_ID)!!

    assertNull(root.contentDescription)
    assertEquals(1, root.childCount)
    assertTrue(
      wrapped.findAccessibilityNodeInfosByText("Scan pairing code", BACKDROP_ID).orEmpty().isEmpty(),
    )
  }

  @Test
  fun forwardsQueriesActionsAndExtraData() {
    val wrapped = host.getAccessibilityNodeProvider()!!
    val byText = wrapped.findAccessibilityNodeInfosByText("pairing", BACKDROP_ID)
    val focused = wrapped.findFocus(AccessibilityNodeInfo.FOCUS_ACCESSIBILITY)!!
    val actionArguments = Bundle()
    val extraArguments = Bundle()
    val extraInfo = AccessibilityNodeInfo.obtain(host, BACKDROP_ID)

    assertEquals("Dismiss pairing dialog", byText!!.single().contentDescription)
    assertEquals("Dismiss pairing dialog", focused.contentDescription)
    assertTrue(wrapped.performAction(BACKDROP_ID, AccessibilityNodeInfo.ACTION_CLICK, actionArguments))
    wrapped.addExtraDataToAccessibilityNodeInfo(
      BACKDROP_ID,
      extraInfo,
      "fixture-key",
      extraArguments,
    )

    assertEquals(BACKDROP_ID, provider.lastActionNode)
    assertSame(actionArguments, provider.lastActionArguments)
    assertEquals(BACKDROP_ID, provider.lastExtraDataNode)
    assertEquals("fixture-key", provider.lastExtraDataKey)
    assertSame(extraArguments, provider.lastExtraDataArguments)
    assertEquals("first", extraInfo.extras.getString("fixture-provider"))
  }

  @Test
  fun cachesByUnderlyingProviderIdentityAndHandlesReplacement() {
    val first = host.getAccessibilityNodeProvider()
    assertSame(first, host.getAccessibilityNodeProvider())

    val replacement = FixtureProvider(host, "replacement")
    host.sourceProvider = replacement
    val second = host.getAccessibilityNodeProvider()

    assertNotSame(first, second)
    assertSame(second, host.getAccessibilityNodeProvider())
    assertTrue(second!!.performAction(BACKDROP_ID, AccessibilityNodeInfo.ACTION_CLICK, null))
    assertEquals(BACKDROP_ID, replacement.lastActionNode)
    assertNull(provider.lastActionNode)

    host.sourceProvider = null
    assertNull(host.getAccessibilityNodeProvider())
    host.sourceProvider = replacement
    assertNotSame(second, host.getAccessibilityNodeProvider())
  }

  private class FixtureHost(context: Context) : WebView(context) {
    var sourceProvider: AccessibilityNodeProvider? = null
    private val pairingAccessibility = PairingBackdropAccessibility(this, "Dismiss pairing dialog")

    override fun getAccessibilityNodeProvider(): AccessibilityNodeProvider? =
      pairingAccessibility.wrap(sourceProvider)
  }

  private class FixtureProvider(
    private val host: WebView,
    private val name: String,
    private val includeScanner: Boolean = true,
  ) : AccessibilityNodeProvider() {
    var lastActionNode: Int? = null
    var lastActionArguments: Bundle? = null
    var lastExtraDataNode: Int? = null
    var lastExtraDataKey: String? = null
    var lastExtraDataArguments: Bundle? = null

    override fun createAccessibilityNodeInfo(virtualViewId: Int): AccessibilityNodeInfo? {
      val info = AccessibilityNodeInfo.obtain(host, virtualViewId)
      info.packageName = host.context.packageName
      info.className = View::class.java.name
      setFullBounds(info)
      when (virtualViewId) {
        BACKDROP_ID -> {
          clickableContainer(info, PAIRING_MARKER)
          info.addChild(host, DOCUMENT_ID)
          if (includeScanner) info.addChild(host, SCANNER_ID)
        }
        GENERIC_DIALOG_ID -> {
          clickableContainer(info, "generic-dialog")
          info.addChild(host, DOCUMENT_ID)
          info.addChild(host, SCANNER_ID)
        }
        ZERO_CHILD_ID -> clickableContainer(info, PAIRING_MARKER)
        ONE_CHILD_ID -> {
          clickableContainer(info, PAIRING_MARKER)
          info.addChild(host, DOCUMENT_ID)
        }
        PARTIAL_BOUNDS_ID -> {
          clickableContainer(info, PAIRING_MARKER)
          info.addChild(host, DOCUMENT_ID)
          info.addChild(host, SCANNER_ID)
          info.setBoundsInScreen(Rect(0, 0, 100, 100))
        }
        DOCUMENT_ID -> {
          info.viewIdResourceName = "application-document"
          if (includeScanner) info.addChild(host, SCANNER_ID)
        }
        SCANNER_ID -> {
          if (!includeScanner) return null
          info.className = "android.widget.Button"
          info.text = "Scan pairing code"
          info.isClickable = true
        }
        else -> return null
      }
      return info
    }

    override fun findAccessibilityNodeInfosByText(
      text: String,
      virtualViewId: Int,
    ): List<AccessibilityNodeInfo> = when (text) {
      "pairing" -> listOfNotNull(createAccessibilityNodeInfo(BACKDROP_ID))
      "Scan pairing code" -> listOfNotNull(createAccessibilityNodeInfo(SCANNER_ID))
      else -> emptyList()
    }

    override fun findFocus(focus: Int): AccessibilityNodeInfo? =
      createAccessibilityNodeInfo(BACKDROP_ID)

    override fun performAction(virtualViewId: Int, action: Int, arguments: Bundle?): Boolean {
      lastActionNode = virtualViewId
      lastActionArguments = arguments
      return action == AccessibilityNodeInfo.ACTION_CLICK
    }

    override fun addExtraDataToAccessibilityNodeInfo(
      virtualViewId: Int,
      info: AccessibilityNodeInfo,
      extraDataKey: String,
      arguments: Bundle?,
    ) {
      lastExtraDataNode = virtualViewId
      lastExtraDataKey = extraDataKey
      lastExtraDataArguments = arguments
      info.extras.putString("fixture-provider", name)
    }

    private fun clickableContainer(info: AccessibilityNodeInfo, marker: String) {
      info.viewIdResourceName = marker
      info.isClickable = true
      info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_CLICK)
    }

    private fun setFullBounds(info: AccessibilityNodeInfo) {
      val location = IntArray(2)
      host.getLocationOnScreen(location)
      info.setBoundsInScreen(
        Rect(location[0], location[1], location[0] + host.width, location[1] + host.height),
      )
    }
  }

  private companion object {
    const val PAIRING_MARKER = "copypaste-pairing-dialog-open"
    const val BACKDROP_ID = 10
    const val DOCUMENT_ID = 11
    const val SCANNER_ID = 12
    const val GENERIC_DIALOG_ID = 13
    const val ZERO_CHILD_ID = 14
    const val ONE_CHILD_ID = 15
    const val PARTIAL_BOUNDS_ID = 16
  }
}
