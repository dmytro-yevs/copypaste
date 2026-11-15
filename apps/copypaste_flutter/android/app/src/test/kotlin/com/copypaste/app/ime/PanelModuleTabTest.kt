package com.copypaste.app.ime

import android.app.Activity
import android.view.View
import android.view.inputmethod.EditorInfo
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.ListView
import com.copypaste.app.ModuleResult
import com.copypaste.app.QuickPasteSurface
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
class PanelModuleTabTest {
    private val activity = Robolectric.buildActivity(Activity::class.java).setup().get()
    private val surface = QuickPasteSurface("copypaste.example", "Library", "search", "use")

    private fun result(id: String, title: String, preview: String, secret: Boolean = false) = JSONObject()
        .put("id", id).put("title", title).put("preview", preview).put("secret", secret)

    private fun deliver(text: String, requireAuth: Boolean = false) = JSONObject().put("kind", "deliver").put(
        "delivery",
        JSONObject().put("text", text).put("require_auth", requireAuth).put("label", "Library"),
    )

    /** Outputs by module command; `use` answers [activation]. */
    private var activation: JSONObject = deliver("Hello")
    private var submitted: JSONObject = deliver("Hi Ann")
    private val runtime = FeatureRuntime { method, params ->
        val operation = params.optJSONObject("operation")
        if (method != "modules" || operation == null) return@FeatureRuntime FeatureRuntime.empty
        val output = when (operation.optString("command")) {
            "search" -> JSONObject().put("kind", "results").put(
                "items",
                JSONArray().put(result("s1", "Greeting", "Hello")).put(result("s2", "PIN", "1234", secret = true)),
            )
            "use" -> activation
            "fill" -> submitted
            else -> JSONObject().put("kind", "message").put("message", "done")
        }
        FeatureRuntime.Answer.Ok("modules", JSONObject().put("json", output.toString()))
    }

    private fun invocations(command: String): List<JSONObject> = runtime.ops("modules")
        .mapNotNull { it.optJSONObject("operation") }
        .filter { it.optString("operation") == "invoke" && it.optString("command") == command }

    private fun editorHost(): Pair<EditText, FakeTabHost> {
        val editor = EditText(activity).apply { activity.setContentView(this) }
        val connection = editor.onCreateInputConnection(EditorInfo())
        return editor to FakeTabHost(FieldPolicy(textEditor = true)) { connection }
    }

    private fun show(host: FakeTabHost): Pair<PanelModuleTab, View> {
        val tab = PanelModuleTab(
            activity, host, runtime.worker, PanelTransforms(activity, host, runtime.worker),
            PanelQueueTab(activity, host, runtime.worker), surface,
        )
        val root = tab.createView(FrameLayout(activity))
        tab.onShown("gr")
        return tab to root
    }

    private fun row(root: View, position: Int): ModuleResult =
        root.findViewById<ListView>(R.id.ime_module_list).adapter.getItem(position) as ModuleResult

    @Test fun searchListsResultsAndATapInsertsTheDelivery() {
        val (editor, host) = editorHost()
        val (_, root) = show(host)

        val search = invocations("search").single()
        assertEquals("copypaste.example", search.getString("id"))
        assertEquals("gr", JSONObject(search.getString("arguments_json")).getString("query"))
        val list = root.findViewById<ListView>(R.id.ime_module_list)
        val secretRow = renderedText(list.adapter.getView(1, null, list))
        assertTrue(secretRow.none { "1234" in it })

        list.performItemClick(list.adapter.getView(0, null, list), 0, 0)

        assertEquals("s1", JSONObject(invocations("use").single().getString("arguments_json")).getString("id"))
        assertEquals("Hello", editor.text.toString())
        assertEquals(0, host.unlocks)
    }

    @Test fun aDeliveryThatRequiresAuthUnlocksFirst() {
        val (editor, host) = editorHost()
        activation = deliver("1234", requireAuth = true)
        val (tab, root) = show(host)

        tab.insert(row(root, 1))

        assertEquals(1, host.unlocks)
        assertEquals("1234", editor.text.toString())
    }

    @Test fun aFormSubmitsItsValuesAndInsertsTheResult() {
        val (editor, host) = editorHost()
        activation = JSONObject().put("kind", "view").put(
            "view",
            JSONObject().put(
                "sections",
                JSONArray().put(
                    JSONObject().put("title", JSONObject.NULL).put(
                        "items",
                        JSONArray().put(
                            JSONObject().put("kind", "form")
                                .put("fields", JSONArray().put(JSONObject().put("id", "name").put("title", "Name").put("kind", "text").put("default", "").put("required", true)))
                                .put("values", JSONObject().put("name", "Ann"))
                                .put("submit", JSONObject().put("title", "Insert").put("command", "fill").put("arguments", JSONObject().put("id", "s1"))),
                        ),
                    ),
                ),
            ),
        )
        val (tab, root) = show(host)

        tab.insert(row(root, 0))
        assertFalse(editor.text.toString() == "Hi Ann")
        root.findViewById<View>(R.id.ime_form_submit).performClick()

        val arguments = JSONObject(invocations("fill").single().getString("arguments_json"))
        assertEquals("s1", arguments.getString("id"))
        assertEquals("Ann", arguments.getJSONObject("values").getString("name"))
        assertEquals("Hi Ann", editor.text.toString())
    }
}
