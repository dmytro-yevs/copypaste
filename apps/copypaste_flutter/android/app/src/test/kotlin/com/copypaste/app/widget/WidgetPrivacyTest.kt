package com.copypaste.app.widget

import com.copypaste.app.JsonRuntimeAccess
import com.copypaste.app.widget.WidgetFilter.Module
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class WidgetPrivacyTest {
    private val now = 1_000_000L
    private val sent = mutableListOf<JSONObject>()

    private fun item(id: String, content: String, extra: JSONObject.() -> Unit = {}) = JSONObject()
        .put("id", id)
        .put("content", content)
        .put("content_type", "text/plain")
        .put("content_class", "text")
        .put("created_at", now - 60_000)
        .put("pinned", false)
        .put("source_app_name", "Notes")
        .apply(extra)

    private val clips = JSONArray()
        .put(item("plain", "line one\n\nline two " + "x".repeat(200)))
        .put(item("secret", "") { put("privacy", JSONObject().put("secret", true)) })
        .put(item("masked", "card 4111") { put("annotations", JSONObject().put("rule_secret", true)) })
        .put(item("expired", "gone") { put("annotations", JSONObject().put("expires_at", now - 1)) })

    private val module = Module("copypaste.example", "Library")

    private val modules = JSONArray().put(
        JSONObject().put("id", module.id).put("title", "Example").put("enabled", true).put("error", JSONObject.NULL).put(
            "surfaces",
            JSONObject().put(
                "quick_paste",
                JSONObject().put("title", module.title).put("search_command", "search").put("activate_command", "use"),
            ),
        ),
    )

    private val results = JSONObject().put("kind", "results").put(
        "items",
        JSONArray()
            .put(JSONObject().put("id", "s1").put("title", "Address").put("preview", "1 Main St"))
            .put(JSONObject().put("id", "s2").put("title", "Bank PIN").put("preview", "4321").put("secret", true))
            .put(JSONObject().put("id", "s3").put("title", "Reply").put("preview", "Hi {{name}}")),
    )

    private val loader = WidgetLoader(
        JsonRuntimeAccess { raw ->
            val request = JSONObject(raw).also { sent += it }
            val data = when (request.getString("method")) {
                "get_private_mode" -> JSONObject().put("private_mode", JSONObject().put("private_mode", true))
                "history_query" -> JSONObject().put("page", JSONObject().put("items", clips))
                "history_counts" -> JSONObject().put("history_counts", JSONObject().put("pinned", 2).put("ranges", JSONArray().put(5)))
                "modules" -> {
                    val operation = request.getJSONObject("params").getJSONObject("operation")
                    val json = if (operation.getString("operation") == "list") modules.toString() else results.toString()
                    JSONObject().put("modules", JSONObject().put("json", json))
                }
                else -> JSONObject().put("empty", JSONObject())
            }
            JSONObject().put("id", 0).put("ok", true).put("data", data).toString()
        },
    ) { now }

    private fun rows(config: WidgetConfig) = (loader.load(config, 5) as WidgetSnapshot.Rows).rows

    @Test fun hiddenShowsOnlyACountAndNeverReadsContent() {
        val snapshot = loader.load(WidgetConfig(privacy = WidgetPrivacy.Hidden), 5)

        assertEquals(WidgetSnapshot.Count(paused = true, count = 7), snapshot)
        assertFalse(sent.any { it.getString("method") == "history_query" })
    }

    @Test fun titlesOnlyShowNoClipBodyAndOnlyModuleResultTitles() {
        val clipRows = rows(WidgetConfig(privacy = WidgetPrivacy.Titles))
        assertTrue(clipRows.all { it.text == null && it.secondary == null })
        assertEquals("Notes", clipRows.first().sourceApp)

        val moduleRows = rows(WidgetConfig(module, WidgetPrivacy.Titles))
        assertEquals(listOf("Address", "Bank PIN", "Reply"), moduleRows.map { it.text })
        assertTrue(moduleRows.all { it.secondary == null })
    }

    @Test fun fullPreviewMasksSecretsDropsExpiredClipsAndCapsText() {
        val rows = rows(WidgetConfig()).associateBy { it.id }

        assertEquals(setOf("plain", "secret", "masked"), rows.keys)
        val preview = rows.getValue("plain").text!!
        assertTrue(preview.startsWith("line one line two"))
        assertEquals(WidgetLoader.MAX_PREVIEW_CHARS + 1, preview.length)
        for (id in listOf("secret", "masked")) {
            val row = rows.getValue(id)
            assertNull(row.text)
            assertTrue(row.masked)
            assertEquals(WidgetTap.AuthenticateAndCopy, row.tap)
        }
        assertEquals(WidgetTap.Copy, rows.getValue("plain").tap)
    }

    @Test fun secretModuleResultsShowNoPreviewAndListWithAnEmptyQuery() {
        val rows = rows(WidgetConfig(filter = module)).associateBy { it.id }

        assertEquals("1 Main St", rows.getValue("s1").secondary)
        assertNull(rows.getValue("s2").secondary)
        assertTrue(rows.getValue("s2").masked)
        assertTrue(rows.values.all { it.moduleId == module.id && it.tap == WidgetTap.Copy })
        val search = sent.map { it.optJSONObject("params")?.optJSONObject("operation") }.last { it?.optString("operation") == "invoke" }!!
        assertEquals("search", search.getString("command"))
        assertEquals("", JSONObject(search.getString("arguments_json")).getString("query"))
    }
}
