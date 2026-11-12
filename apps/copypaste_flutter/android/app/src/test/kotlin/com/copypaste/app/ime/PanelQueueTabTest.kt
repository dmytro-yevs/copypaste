package com.copypaste.app.ime

import android.app.Activity
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView
import com.copypaste.app.R
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class PanelQueueTabTest {
    private val activity = Robolectric.buildActivity(Activity::class.java).setup().get()

    private fun entry(id: String, kind: String = "text", preview: String = id) = JSONObject()
        .put("entry_id", id).put("position_label", "1").put("status", "pending").put("content_kind", kind)
        .put("preview", preview).put("masked", false).put("edited", false).put("added_via", "history_add")

    private fun state(vararg entries: JSONObject) = JSONObject()
        .put("revision", 1).put("collecting", false).put("phase", "active").put("full", false)
        .put("entries", JSONArray(entries.toList()))
        .put("settings", JSONObject().put("enabled", true).put("separator", "new_line").put("move_to_next_field", false))

    private fun runtime(state: JSONObject) = FeatureRuntime { method, op ->
        when {
            method != "paste_queue" -> FeatureRuntime.empty
            op.optString("op") == "deliver" -> FeatureRuntime.Answer.Ok(
                "queue_delivery",
                JSONObject().put("token", "t1").put("entry_ids", JSONArray().put("a")).put("text", "alpha")
                    .put("notices", JSONObject().put("unavailable", 0)).put("finished", false),
            )
            else -> FeatureRuntime.Answer.Ok("paste_queue", state)
        }
    }

    @Test fun insertNextMarksTheEntryPastedOnlyWhenTheEditorAcceptsTheCommit() {
        val editor = EditText(activity).apply { activity.setContentView(this) }
        val info = EditorInfo()
        val connection = editor.onCreateInputConnection(info)
        val accepted = runtime(state(entry("a"), entry("b")))
        val tab = PanelQueueTab(activity, FakeTabHost(FieldPolicy.of(info)) { connection }, accepted.worker)
        tab.createView(FrameLayout(activity))
        tab.onShown("")

        tab.insertNext()

        assertEquals("alpha", editor.text.toString())
        assertEquals("return", accepted.op("paste_queue", "deliver")!!.getString("mode"))
        assertEquals("inserted", accepted.op("paste_queue", "confirm")!!.getString("outcome"))

        val rejecting = object : BaseInputConnection(TextView(activity), true) {
            override fun commitText(text: CharSequence?, newCursorPosition: Int): Boolean = false
        }
        val refused = runtime(state(entry("a")))
        val host = FakeTabHost(FieldPolicy(textEditor = true)) { rejecting }
        val second = PanelQueueTab(activity, host, refused.worker)
        second.createView(FrameLayout(activity))
        second.onShown("")

        second.insertNext()

        assertEquals("rejected", refused.op("paste_queue", "confirm")!!.getString("outcome"))
        assertEquals(listOf(InsertOutcome.REJECTED), host.outcomes)
    }

    @Test fun insertNextOnAnImageIsUnavailableWithItsReasonInAFieldWithoutImageSupport() {
        val runtime = runtime(state(entry("a", kind = "image", preview = "")))
        val host = FakeTabHost(FieldPolicy(textEditor = true)) { null }
        val tab = PanelQueueTab(activity, host, runtime.worker)
        val root = tab.createView(FrameLayout(activity))
        tab.onShown("")
        val reason = activity.getString(R.string.ime_image_not_accepted)

        val insertNext = root.findViewById<LinearLayout>(R.id.ime_queue_actions).getChildAt(0)
        assertTrue(insertNext.alpha < 1f)
        assertTrue(reason in insertNext.contentDescription.toString())
        assertTrue(reason in root.findViewById<TextView>(R.id.ime_queue_status).text.toString())

        insertNext.performClick()

        assertNull(runtime.op("paste_queue", "deliver"))
        assertEquals(listOf(reason), host.notices)
    }
}
