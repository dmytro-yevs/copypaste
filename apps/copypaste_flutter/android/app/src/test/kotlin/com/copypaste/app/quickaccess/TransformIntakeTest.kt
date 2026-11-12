package com.copypaste.app.quickaccess

import android.app.Activity
import android.content.Intent
import android.view.ViewGroup
import android.widget.ListView
import android.widget.TextView
import org.robolectric.RuntimeEnvironment
import com.copypaste.app.R
import com.copypaste.app.RuntimeAccessWorker
import com.copypaste.app.TransformActivity
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class TransformIntakeTest {
    private val context = RuntimeEnvironment.getApplication()

    @After fun restoreRuntime() {
        QuickAccessRuntime.factory = RuntimeAccessWorker::native
    }

    @Test fun replaceIsOfferedOnlyForAnEditableSelection() {
        install()
        assertEquals(listOf("Replace", "Copy", "Save"), footer(launch(readOnly = false)))
        assertEquals(listOf("Copy", "Save"), footer(launch(readOnly = true)))
        assertEquals(listOf(OutputAction.COPY), OutputAction.available(editable = false, sensitive = true))
    }

    @Test fun replaceReturnsTheResultToTheField() {
        val runtime = install()
        val activity = launch(readOnly = false)
        val list = activity.findViewById<ListView>(R.id.qa_sheet_list)

        list.performItemClick(null, 1, 1)
        activity.findViewById<ViewGroup>(R.id.qa_sheet_footer).getChildAt(0).performClick()

        val run = runtime.requests.last().getJSONObject("params").getJSONObject("op")
        assertEquals("run", run.getString("op"))
        assertEquals("return", run.getString("action"))
        assertEquals("hello", run.getJSONObject("source").getString("text"))
        assertEquals(Activity.RESULT_OK, shadowOf(activity).resultCode)
        assertEquals("HELLO", shadowOf(activity).resultIntent.getStringExtra(Intent.EXTRA_PROCESS_TEXT))
    }

    @Test fun freeTextTransformsAreReachedOnlyThroughPipelines() {
        val catalogue = JSONArray()
            .put(transform("case.upper", "case"))
            .put(transform("text.replace", "clean").put("flags", JSONObject().put("needs_free_text", true)))
            .put(transform("hash.md5", "hash"))
        val preferences = JSONObject()
            .put("favorites", JSONArray().put(JSONObject().put("kind", "transform").put("id", "hash.md5")))
            .put("hidden_transforms", JSONArray().put("case.upper"))
        val pipelines = JSONArray().put(JSONObject().put("id", "p1").put("name", "Tidy").put("steps", JSONArray()))

        val sections = TransformPalette.build(catalogue, preferences, pipelines)

        assertEquals(
            listOf(R.string.quick_access_transform_favorites, R.string.quick_access_transform_pipelines, R.string.quick_access_transform_category_hash),
            sections.map { it.title },
        )
        assertEquals(listOf("hash.md5", "p1", "hash.md5"), sections.flatMap { section -> section.entries.map { it.id } })
    }

    private fun install(): FakeQuickAccessRuntime = FakeQuickAccessRuntime { method, op ->
        when ("$method/$op") {
            "transforms/catalogue" -> JSONObject().put("transform_catalogue", JSONArray().put(transform("case.upper", "case")))
            "transforms/preferences" -> JSONObject().put("transform_preferences", JSONObject())
            "transforms/pipelines" -> JSONObject().put("pipelines", JSONArray())
            "transforms/preview" -> JSONObject().put(
                "transform_preview",
                JSONObject().put("no_changes", false).put("truncated", false).put("sensitive", false)
                    .put("steps", JSONArray().put(JSONObject().put("output_prefix", "HELLO").put("warnings", JSONArray()))),
            )
            "transforms/run" -> JSONObject().put("delivered", JSONObject().put("text", "HELLO"))
            else -> null
        }
    }.install()

    private fun launch(readOnly: Boolean): Activity {
        val intent = Intent(context, TransformActivity::class.java)
            .setAction(Intent.ACTION_PROCESS_TEXT)
            .putExtra(Intent.EXTRA_PROCESS_TEXT, "hello")
            .putExtra(Intent.EXTRA_PROCESS_TEXT_READONLY, readOnly)
        return Robolectric.buildActivity(TransformActivity::class.java, intent).setup().get()
    }

    private fun footer(activity: Activity): List<String> {
        val footer = activity.findViewById<ViewGroup>(R.id.qa_sheet_footer)
        return (0 until footer.childCount).map { (footer.getChildAt(it) as TextView).text.toString() }
    }

    private fun transform(id: String, category: String): JSONObject =
        JSONObject().put("id", id).put("version", 1).put("category", category).put("name", id)
            .put("description", "").put("params", JSONArray()).put("flags", JSONObject())
}
