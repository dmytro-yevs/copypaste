package com.copypaste.app.ime

import android.app.Activity
import android.text.InputType
import android.view.View
import android.view.inputmethod.EditorInfo
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ListView
import com.copypaste.app.R
import org.json.JSONArray
import org.json.JSONObject
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
class PanelTransformsTest {
    private val activity = Robolectric.buildActivity(Activity::class.java).setup().get()

    private fun transform(id: String, name: String, freeText: Boolean = false) = JSONObject()
        .put("id", id).put("version", 1).put("category", "case").put("name", name).put("description", "")
        .put("keywords", JSONArray()).put("params", JSONArray())
        .put("flags", JSONObject().put("needs_free_text", freeText))

    private fun runtime(output: () -> String) = FeatureRuntime { method, op ->
        if (method != "transforms") return@FeatureRuntime FeatureRuntime.empty
        when (op.optString("op")) {
            "catalogue" -> FeatureRuntime.Answer.Ok(
                "transform_catalogue",
                JSONArray().put(transform("case.upper", "UPPERCASE")).put(transform("text.replace", "Find and replace", freeText = true)),
            )
            "pipelines" -> FeatureRuntime.Answer.Ok(
                "pipelines",
                JSONArray().put(
                    JSONObject().put("id", "p1").put("name", "Tidy names")
                        .put("steps", JSONArray().put(JSONObject().put("transform_id", "text.replace").put("version", 1).put("params_json", "{}").put("enabled", true))),
                ),
            )
            "preferences" -> FeatureRuntime.Answer.Ok("transform_preferences", JSONObject())
            "run" -> FeatureRuntime.Answer.Ok("delivered", JSONObject().put("text", output()))
            else -> FeatureRuntime.empty
        }
    }

    private class Shown(val tab: PanelTransforms, val root: View) {
        val list: ListView get() = root.findViewById(R.id.ime_transforms_list)
        val names: List<String> get() = (0 until list.adapter.count).map { renderedText(list.adapter.getView(it, null, list)).first() }

        fun choose(name: String) {
            val position = names.indexOf(name)
            list.performItemClick(list.adapter.getView(position, null, list), position, position.toLong())
        }

        fun insert() = root.findViewById<LinearLayout>(R.id.ime_transforms_actions).getChildAt(0).performClick()
    }

    private fun show(host: FakeTabHost, runtime: FeatureRuntime): Shown {
        val tab = PanelTransforms(activity, host, runtime.worker, activity.getSharedPreferences("transforms-test", 0))
        val root = tab.createView(FrameLayout(activity))
        tab.onShown("")
        return Shown(tab, root)
    }

    @Test fun insertReplacesExactlyTheSelection() {
        val editor = EditText(activity).apply {
            setText("Hello world")
            activity.setContentView(this)
            setSelection(6, 11)
        }
        val info = EditorInfo()
        val connection = editor.onCreateInputConnection(info)
        val runtime = runtime { "WORLD" }
        val shown = show(FakeTabHost(FieldPolicy.of(info)) { connection }, runtime)

        shown.choose("UPPERCASE")
        shown.insert()

        val run = runtime.op("transforms", "run")!!
        assertEquals("text", run.getJSONObject("source").getString("kind"))
        assertEquals("world", run.getJSONObject("source").getString("text"))
        assertEquals("Hello WORLD", editor.text.toString())
    }

    @Test fun freeTextTransformsAreOfferedOnlyThroughSavedPipelines() {
        val shown = show(FakeTabHost(FieldPolicy(textEditor = true)) { null }, runtime { "" })

        assertTrue("UPPERCASE" in shown.names)
        assertTrue("Tidy names" in shown.names)
        assertFalse("Find and replace" in shown.names)
    }

    @Test fun largeResultsWarnFirstAndOversizedOnesOfferCopyInstead() {
        val editor = EditText(activity).apply { activity.setContentView(this) }
        val info = EditorInfo()
        val connection = editor.onCreateInputConnection(info)
        var output = "x".repeat(PanelTransforms.WARN_INSERT_BYTES + 1)
        val host = FakeTabHost(FieldPolicy.of(info)) { connection }
        val runtime = runtime { output }
        val shown = show(host, runtime)
        shown.choose("UPPERCASE")

        shown.insert()

        assertTrue(host.sheets.single().first.startsWith(activity.getString(R.string.ime_transforms_large, "").trimEnd()))
        assertEquals("", editor.text.toString())

        output = "x".repeat(PanelTransforms.MAX_INSERT_BYTES + 1)
        shown.insert()

        assertEquals(activity.getString(R.string.ime_transforms_too_large), host.notices.last())
        assertTrue(shown.root.findViewById<LinearLayout>(R.id.ime_transforms_actions).getChildAt(0).alpha < 1f)
        host.noticeAction!!.invoke()
        assertEquals("copy", runtime.op("transforms", "run")!!.getString("action"))
        assertEquals("", editor.text.toString())
    }

    @Test fun passwordFieldsOfferNoTransformAction() {
        val runtime = FakeRuntime(listOf(FakeRuntime.item("a", "token")))
        val panel = controller(activity, runtime)
        registerPanelFeatures(activity, panel, runtime.worker)
        panel.createView()
        val label = activity.getString(R.string.ime_action_transform)
        val password = FieldPolicy.of(EditorInfo().apply { inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD })

        panel.onShow(FieldPolicy(textEditor = true))
        assertTrue(panel.actionsFor(panel.history.loaded.single()).any { it.label == label })

        panel.onShow(password)
        assertFalse(panel.actionsFor(panel.history.loaded.single()).any { it.label == label })
    }
}
