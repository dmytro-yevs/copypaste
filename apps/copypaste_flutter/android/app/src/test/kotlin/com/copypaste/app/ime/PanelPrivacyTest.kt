package com.copypaste.app.ime

import android.app.Activity
import android.text.InputType
import android.view.inputmethod.EditorInfo
import android.widget.EditText
import android.widget.FrameLayout
import com.copypaste.app.DeviceAuthenticator
import com.copypaste.app.QuickAccessActivity
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class PanelPrivacyTest {
    private val activity = Robolectric.buildActivity(Activity::class.java).setup().get()

    private fun field(inputType: Int, imeOptions: Int = 0) = FieldPolicy.of(
        EditorInfo().apply {
            this.inputType = inputType
            this.imeOptions = imeOptions
        },
    )

    @Test fun passwordFieldsMaskEveryPreviewSkipCatchUpAndNeverPromoteUsage() {
        val runtime = FakeRuntime(listOf(FakeRuntime.item("a", "hunter2-token")))
        val clipboard = FakeClipboard(stamp = 42L)
        val editor = EditText(activity).apply { activity.setContentView(this) }
        val connection = editor.onCreateInputConnection(EditorInfo())
        val host = FakeHost(connection)
        val panel = controller(activity, runtime, host, clipboard)
        panel.createView()
        val password = field(InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD)

        panel.onShow(password)
        val row = panel.history.adapter.getView(0, null, FrameLayout(activity))

        assertTrue(password.maskedMode)
        assertTrue(host.secure)
        assertTrue(renderedText(row).none { "hunter2" in it })
        assertEquals(0, clipboard.captures)

        panel.onItemTapped(panel.history.loaded.single())

        assertEquals("hunter2-token", editor.text.toString())
        assertFalse("copy" in runtime.methods)
        assertFalse("pin" in runtime.methods)
    }

    @Test fun noLearningFieldsNeverCapture() {
        val clipboard = FakeClipboard(stamp = 7L)
        val panel = controller(activity, FakeRuntime(), clipboard = clipboard)
        panel.createView()
        val incognito = field(InputType.TYPE_CLASS_TEXT, EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING)

        panel.onShow(incognito)

        assertTrue(incognito.incognito)
        assertFalse(incognito.recordsUsage)
        assertEquals(0, clipboard.captures)
    }

    @Test fun theLockedStateRendersNoClipText() {
        val runtime = FakeRuntime(listOf(FakeRuntime.item("a", "private note"), FakeRuntime.item("b", "pinned note", pinned = true)))
        val panel = controller(activity, runtime, locked = true)
        val root = panel.createView()

        panel.onShow(field(InputType.TYPE_CLASS_TEXT))

        assertFalse("history_query" in runtime.methods)
        assertEquals(0, panel.history.adapter.count)
        assertTrue(renderedText(root).none { "note" in it })
    }

    @Test fun secretsNeedDeviceAuthenticationAgainAfterThePanelHides() {
        val runtime = FakeRuntime(listOf(FakeRuntime.item("s", "s3cret", secret = true)))
        val editor = EditText(activity).apply { activity.setContentView(this) }
        val host = FakeHost(editor.onCreateInputConnection(EditorInfo()))
        val panel = controller(activity, runtime, host)
        panel.createView()
        val text = field(InputType.TYPE_CLASS_TEXT)
        panel.onShow(text)
        val secret = panel.history.loaded.single()

        panel.onItemTapped(secret)

        val intent = host.launched.single()
        assertEquals(QuickAccessActivity.ACTION_AUTHENTICATE, intent.action)
        assertEquals(QuickAccessActivity::class.java.name, intent.component!!.className)
        assertFalse("get" in runtime.methods)

        panel.onAuthenticated(DeviceAuthenticator.Outcome.Success)
        assertEquals("s3cret", editor.text.toString())

        panel.onHide()
        panel.onShow(text)
        panel.onItemTapped(panel.history.loaded.single())

        assertEquals(2, host.launched.size)
    }

    @Test fun visibilityFollowsPrivacyAnnotationsAndExpiry() {
        val item = FakeRuntime.item("r", "x").put(
            "annotations",
            org.json.JSONObject().put("rule_secret", true).put("expires_at", 5_000L),
        )
        val visibility = ClipVisibility.of(item)

        assertEquals(ClipPresentation.MASKED, visibility.presentationAt(1_000L))
        assertEquals(ClipPresentation.HIDDEN, visibility.presentationAt(5_000L))
        assertEquals(ClipPresentation.MASKED, ClipVisibility(secret = false).presentationAt(0L, maskedMode = true))
    }
}
