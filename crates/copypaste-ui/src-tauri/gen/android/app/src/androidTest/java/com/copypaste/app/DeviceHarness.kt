package com.copypaste.app

import android.app.Instrumentation
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.ParcelFileDescriptor
import android.view.accessibility.AccessibilityNodeInfo
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.BySelector
import androidx.test.uiautomator.StaleObjectException
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.UiObject2
import androidx.test.uiautomator.Until
import java.io.ByteArrayOutputStream
import java.io.FileInputStream
import java.nio.charset.StandardCharsets
import java.util.regex.Pattern
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue

/**
 * The WebView publishes each control's accessible name as the node's `text`,
 * not as `content-desc`, so every selector here is text-based. Chromium reports
 * a node scrolled outside the viewport with zero bounds rather than omitting
 * it, which is why [tap] scrolls before it clicks: `UiObject2.click()` on a
 * zero-bounds node clicks the top-left of the screen instead of failing.
 */
internal class DeviceHarness {
    val instrumentation: Instrumentation = InstrumentationRegistry.getInstrumentation()
    val context: Context = instrumentation.targetContext
    val device: UiDevice = UiDevice.getInstance(instrumentation)
    private val packageName = context.packageName

    fun launch() {
        val intent = context.packageManager.getLaunchIntentForPackage(packageName)
        assertNotNull("launch intent", intent)
        intent!!.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_REORDER_TO_FRONT)
        context.startActivity(intent)
        assertTrue("app did not reach foreground", device.wait(Until.hasObject(By.pkg(packageName)), LAUNCH_MS))
        assertTrue("WebView never rendered the shell", awaitText("CopyPaste", LAUNCH_MS))
    }

    /**
     * Fires a share-style intake and returns once the daemon has stored it.
     *
     * Both the count and [marker] have to settle. Waiting on the count alone
     * let a *previous* intake that was still in flight satisfy the wait, and
     * the test then went looking for a clip the daemon had not seen yet.
     * [marker] defaults to the text, and is given explicitly for a clip whose
     * plaintext the list is required to withhold.
     */
    fun capture(text: String, action: String = Intent.ACTION_SEND, marker: String = text) {
        val before = itemCount()
        deliver(text, action)
        foreground()
        openHistory()
        assertTrue(
            "history did not grow after intake of $action",
            waitUntil(CAPTURE_MS) { itemCount() > before },
        )
        tap("Devices")
        tap("History")
        assertTrue("the $action intake never reached the list: '$marker'", awaitText(marker, CAPTURE_MS))
    }

    /**
     * Intake goes through `am start`, not `startActivitySync`: IntakeActivity
     * finishes inside `onCreate`, so there is no resumed activity to
     * synchronise on, and a second call arrives at the instance still tearing
     * down and is dropped — which silently lost the PROCESS_TEXT half of the
     * ladder while the SEND half passed.
     */
    private fun deliver(text: String, action: String) {
        val extra = if (action == Intent.ACTION_PROCESS_TEXT) {
            Intent.EXTRA_PROCESS_TEXT
        } else {
            Intent.EXTRA_TEXT
        }
        require(text.matches(CANARY)) { "an intake canary must be shell-safe: $text" }
        val result = shell("am start -W -a $action -t text/plain -n $packageName/.IntakeActivity --es $extra $text")
        assertTrue("am refused the $action intake: $result", result.contains("Starting"))
    }

    /** The count the History header renders ("N items"), or -1 while it is
     *  absent. Polling this is what makes an intake observable without
     *  assuming the clip's own text ever becomes visible. */
    fun itemCount(): Int {
        openHistory()
        val node = device.wait(Until.findObject(By.text(ITEM_COUNT)), SHORT_MS) ?: return -1
        return try {
            node.text.substringBefore(' ').toIntOrNull() ?: -1
        } catch (_: StaleObjectException) {
            -1
        }
    }

    fun openHistory() {
        find("History", DEFAULT_MS)?.click()
        device.waitForIdle()
        // A cold start has to unlock the store before the list exists at all.
        device.wait(Until.hasObject(By.text(ITEM_COUNT)), LAUNCH_MS)
    }

    private fun foreground() {
        val intent = context.packageManager.getLaunchIntentForPackage(packageName)
        assertNotNull("launch intent", intent)
        intent!!.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_REORDER_TO_FRONT)
        context.startActivity(intent)
        assertTrue("app did not return to foreground", waitUntil(FOREGROUND_MS) { device.currentPackageName == packageName })
    }

    /** Clicks the control whose accessible name contains [label], scrolling it
     *  into the viewport first when the WebView has parked it outside. */
    fun tap(label: String, timeoutMs: Long = DEFAULT_MS) {
        val node = findOnScreen(label, timeoutMs)
        assertNotNull("no reachable control named '$label'. ${onScreen()}", node)
        node!!.click()
        device.waitForIdle()
    }

    fun reveal(label: String, timeoutMs: Long = DEFAULT_MS) {
        assertNotNull("no reachable control named '$label'. ${onScreen()}", findOnScreen(label, timeoutMs))
    }

    fun setField(id: String, value: String, timeoutMs: Long = DEFAULT_MS) {
        val selector = By.res(Pattern.compile("(^|.*/)${Pattern.quote(id)}$")).clazz("android.widget.EditText")
        val field = device.wait(Until.findObject(selector), timeoutMs)
        assertNotNull("no input with id '$id'. ${onScreen()}", field)
        field!!.text = value
        assertTrue("input '$id' did not retain its value", waitUntil(timeoutMs) {
            try {
                device.findObject(selector)?.text == value
            } catch (_: StaleObjectException) {
                false
            }
        })
    }

    /** For a word that is also a substring of the prose around it: the reveal
     *  confirmation's title contains "Reveal", and it is not the button. */
    fun tapExact(label: String, timeoutMs: Long = DEFAULT_MS) {
        val node = device.wait(Until.findObject(By.text(label).clickable(true)), timeoutMs)
        assertNotNull("no clickable control named exactly '$label'. ${onScreen()}", node)
        node!!.click()
        device.waitForIdle()
    }

    fun tapEnabledExact(label: String, timeoutMs: Long = DEFAULT_MS) {
        val node = device.wait(
            Until.findObject(By.text(label).clickable(true).enabled(true)),
            timeoutMs,
        )
        assertNotNull("no enabled control named exactly '$label'. ${onScreen()}", node)
        node!!.click()
        device.waitForIdle()
    }

    fun tapCheckable(label: String, timeoutMs: Long = DEFAULT_MS) {
        val node = device.wait(Until.findObject(By.text(label).clickable(true)), timeoutMs)
        assertNotNull("no checkable control named '$label'. ${onScreen()}", node)
        node!!.click()
        device.waitForIdle()
    }

    /**
     * A tab activates on focus, and focusing one that is off to the side
     * scrolls the strip under the finger, so the neighbour is what ends up
     * selected. Confirming the pane and tapping again is what makes the
     * navigation deterministic.
     */
    fun selectTab(label: String, paneMarker: String) {
        repeat(TAB_TRIES) {
            assertTrue("no tab node '$label'", activate(label))
            if (awaitText(paneMarker, TAB_SETTLE_MS)) return
        }
        throw AssertionError("tab '$label' never opened a pane showing '$paneMarker'. ${onScreen()}")
    }

    fun openSetting(title: String) {
        require(title.matches(Regex("[A-Za-z ]+")))
        assertTrue("settings search was absent", waitUntil(DEFAULT_MS) {
            try {
                val field = device.findObjects(By.clazz("android.widget.EditText"))
                    .firstOrNull { !it.visibleBounds.isEmpty }
                    ?: return@waitUntil false
                field.text = title
                true
            } catch (_: StaleObjectException) {
                false
            }
        })
        val result = device.wait(
            Until.findObject(By.textContains(title).clazz("android.widget.Button")),
            DEFAULT_MS,
        )
        assertNotNull("settings search found no result for '$title'. ${onScreen()}", result)
        result!!.click()
        device.waitForIdle()
    }

    private fun activate(label: String): Boolean {
        val root = instrumentation.uiAutomation.rootInActiveWindow ?: return false
        val node = root.findAccessibilityNodeInfosByText(label)
            ?.firstOrNull { it.text?.toString() == label }
            ?: return false
        return node.performAction(AccessibilityNodeInfo.ACTION_CLICK)
    }

    /** What the screen was offering when an assertion gave up. A failure that
     *  only says which name was missing costs a whole emulator run to place. */
    fun onScreen(): String =
        LABEL.matcher(hierarchy()).let { matcher ->
            val seen = LinkedHashSet<String>()
            while (matcher.find()) matcher.group(1)?.takeIf { it.isNotBlank() }?.let { seen.add(it) }
            "On screen: " + seen.joinToString(" | ").take(3_000)
        }

    /**
     * The WebView drops a node that is scrolled out of its container from the
     * accessibility tree entirely, so "not found" and "not yet scrolled to"
     * look identical. The row-action sheet is the case that matters: on a
     * 1080x2400 screen it opens showing Copy and Open, with Pin, Delete and
     * Cancel below its own fold.
     */
    private fun findOnScreen(label: String, timeoutMs: Long): UiObject2? {
        onScreenNow(label, timeoutMs)?.let { return it }
        val raw = instrumentation.uiAutomation.rootInActiveWindow
            ?.findAccessibilityNodeInfosByText(label)
            ?.firstOrNull { it.text?.toString()?.contains(label) == true }
        val show = AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.id
        if (raw?.performAction(show) == true) {
            device.waitForIdle()
            onScreenNow(label, SHORT_MS)?.let { return it }
        }
        repeat(SCROLL_TRIES) {
            // Innermost first: a modal's own scroller, not the list behind it.
            val scroller = device.findObjects(By.scrollable(true))
                .minByOrNull { it.visibleBounds.width() * it.visibleBounds.height() }
                ?: return null
            // Only the bottom edge is inset, and only far enough to clear the
            // home gesture. Insetting all four (`setGestureMargin`) leaves the
            // row-action sheet's 243px-tall scroller with nothing to swipe in.
            scroller.setGestureMargins(0, 0, 0, HOME_GESTURE_PX)
            val bounds = scroller.visibleBounds
            device.swipe(
                bounds.centerX(),
                bounds.bottom - HOME_GESTURE_PX,
                bounds.centerX(),
                bounds.top + 1,
                SWIPE_STEPS,
            )
            device.waitForIdle()
            onScreenNow(label, SHORT_MS)?.let { return it }
        }
        return null
    }

    private fun onScreenNow(label: String, timeoutMs: Long): UiObject2? =
        find(label, timeoutMs)?.takeIf { !it.visibleBounds.isEmpty }

    fun find(label: String, timeoutMs: Long = DEFAULT_MS): UiObject2? =
        device.wait(Until.findObject(named(label)), timeoutMs)

    /** Accessible names surface as `text`; a handful of decorative nodes only
     *  carry `content-desc`, so both are accepted. */
    private fun named(label: String): BySelector = By.textContains(label)

    fun awaitText(text: String, timeoutMs: Long = DEFAULT_MS): Boolean =
        device.wait(Until.hasObject(By.textContains(text)), timeoutMs)

    fun awaitGone(text: String, timeoutMs: Long = DEFAULT_MS): Boolean =
        device.wait(Until.gone(By.textContains(text)), timeoutMs)

    fun assertVisible(text: String, timeoutMs: Long = DEFAULT_MS) =
        assertTrue("never became visible: '$text'. ${onScreen()}", awaitText(text, timeoutMs))

    fun assertAbsent(text: String) =
        assertFalse("unexpectedly visible: '$text'", device.hasObject(By.textContains(text)))

    fun forceStopAndLaunch() {
        shell("am force-stop $packageName")
        assertTrue("process survived force-stop", waitUntil(FOREGROUND_MS) { shell("pidof $packageName").isBlank() })
        launch()
    }

    fun clipboardText(): String {
        var text = ""
        // getPrimaryClip must run on a looper thread, and only the foreground
        // app may read the clipboard at all.
        instrumentation.runOnMainSync {
            text = context.getSystemService(ClipboardManager::class.java)
                .primaryClip?.getItemAt(0)?.coerceToText(context)?.toString().orEmpty()
        }
        return text
    }

    fun shell(command: String): String {
        val descriptor: ParcelFileDescriptor = instrumentation.uiAutomation.executeShellCommand(command)
        return FileInputStream(descriptor.fileDescriptor).use { input ->
            String(input.readBytes(), StandardCharsets.UTF_8).trim()
        }.also { descriptor.close() }
    }

    /**
     * A clip's plaintext must not reach anywhere a second app can read it.
     *
     * UiAutomator echoes every selector it is given at debug level, so a
     * canary this suite has already searched for is in the log by the time it
     * is checked. Those tags are silenced rather than the check weakened: they
     * belong to the harness, and nothing the product logs is hidden by them.
     */
    fun assertNoCanaryLeak(canary: String) {
        val logs = shell("logcat -d -v threadtime UiDevice:S UiObject2:S UiAutomation:S *:V")
        assertFalse("clipboard plaintext leaked to logcat", logs.contains(canary))
        assertFalse("clipboard plaintext leaked to window metadata", shell("dumpsys window windows").contains(canary))
    }

    /** Polls until [supplier] yields a non-null value, or gives up. */
    fun <T> waitFor(timeoutMs: Long, supplier: () -> T?): T? {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < deadline) {
            supplier()?.let { return it }
            Thread.sleep(POLL_MS)
        }
        return supplier()
    }

    fun waitUntil(timeoutMs: Long, predicate: () -> Boolean): Boolean {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < deadline) {
            if (predicate()) return true
            Thread.sleep(POLL_MS)
        }
        return predicate()
    }

    fun hierarchy(): String = ByteArrayOutputStream().use { output ->
        device.dumpWindowHierarchy(output)
        output.toString(StandardCharsets.UTF_8.name())
    }

    private companion object {
        const val LAUNCH_MS = 90_000L
        const val CAPTURE_MS = 90_000L
        const val FOREGROUND_MS = 20_000L
        const val DEFAULT_MS = 20_000L
        const val SHORT_MS = 5_000L
        const val POLL_MS = 250L
        const val SCROLL_TRIES = 6
        const val SWIPE_STEPS = 20
        const val HOME_GESTURE_PX = 90
        const val TAB_TRIES = 4
        const val TAB_SETTLE_MS = 6_000L
        val CANARY: Regex = Regex("[A-Za-z0-9._-]+")
        val ITEM_COUNT: java.util.regex.Pattern = java.util.regex.Pattern.compile("\\d+ items?")
        val LABEL: java.util.regex.Pattern = java.util.regex.Pattern.compile("text=\"([^\"]*)\"")
    }
}
