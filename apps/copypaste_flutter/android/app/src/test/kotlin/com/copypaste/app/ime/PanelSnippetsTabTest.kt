package com.copypaste.app.ime

import android.app.Activity
import android.view.inputmethod.EditorInfo
import android.widget.EditText
import android.widget.FrameLayout
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class PanelSnippetsTabTest {
    private val activity = Robolectric.buildActivity(Activity::class.java).setup().get()
    private val snippet = PanelSnippet(id = "s1", title = "Greeting", abbreviation = "hi", template = false, pinned = false, concealed = false)

    /** Output "Hi , 👋🏽 bye" with its cursor right after "Hi " (seven graphemes, ten UTF-16 units, before the end). */
    private val runtime = FeatureRuntime { method, op ->
        when {
            method != "snippets" -> FeatureRuntime.empty
            op.optString("op") in setOf("use", "render") -> FeatureRuntime.Answer.Ok(
                if (op.optString("op") == "use") "delivered" else "rendered",
                JSONObject().put("text", "Hi , 👋🏽 bye").put("cursor_graphemes_from_end", 7),
            )
            else -> FeatureRuntime.empty
        }
    }

    private fun tab(host: FakeTabHost): PanelSnippetsTab {
        val transforms = PanelTransforms(activity, host, runtime.worker)
        val tab = PanelSnippetsTab(activity, host, runtime.worker, transforms, PanelQueueTab(activity, host, runtime.worker))
        tab.createView(FrameLayout(activity))
        return tab
    }

    @Test fun aTemplateCursorLeavesTheCaretAtThePlaceholder() {
        val editor = EditText(activity).apply { activity.setContentView(this) }
        val info = EditorInfo()
        val connection = editor.onCreateInputConnection(info)
        val host = FakeTabHost(FieldPolicy.of(info)) { connection }
        host.insert.onSelectionChanged(0, 0)

        tab(host).insert(snippet)

        assertEquals("Hi , 👋🏽 bye", editor.text.toString())
        assertEquals(3, editor.selectionStart)
        assertEquals(3, editor.selectionEnd)
        assertEquals("return", runtime.op("snippets", "use")!!.getString("delivery"))
    }

    @Test fun privateFieldsRenderTheSnippetWithoutCountingAUse() {
        val editor = EditText(activity).apply {
            imeOptions = EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING
            activity.setContentView(this)
        }
        val info = EditorInfo()
        val connection = editor.onCreateInputConnection(info)
        val host = FakeTabHost(FieldPolicy.of(info)) { connection }
        assertFalse(host.field.recordsUsage)

        tab(host).insert(snippet)

        assertEquals("final",runtime.op("snippets", "render")!!.getString("purpose"))
        assertFalse(runtime.ops("snippets").any { it.optString("op") == "use" })
    }
}
