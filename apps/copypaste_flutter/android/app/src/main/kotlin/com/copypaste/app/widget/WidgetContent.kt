package com.copypaste.app.widget

import com.copypaste.app.ClipPresentation
import com.copypaste.app.ClipVisibility
import com.copypaste.app.RuntimeAccess
import com.copypaste.app.RuntimeErrorCode
import com.copypaste.app.RuntimeReply
import org.json.JSONArray
import org.json.JSONObject

internal enum class WidgetKind { Text, Link, Image, File, Color, Code, Snippet }

/** What tapping a row does in the quick-access activity. */
internal enum class WidgetTap {
    Copy,

    /** Secrets and concealed snippets: verify the device owner first. */
    AuthenticateAndCopy,

    /** A template snippet needs its fields filled in. */
    FillTemplate,
}

/**
 * One row as the widget may show it. [text] and [secondary] already obey the
 * privacy level and masking: a field the level forbids is null, never blank
 * content.
 */
internal data class WidgetRow(
    val id: String,
    val snippet: Boolean,
    val kind: WidgetKind,
    /** Clip preview or snippet title. */
    val text: String?,
    /** Snippet body preview. */
    val secondary: String?,
    val sourceApp: String?,
    val createdAtMs: Long,
    val pinned: Boolean,
    val masked: Boolean,
    val tap: WidgetTap,
)

internal sealed interface WidgetSnapshot {
    /** The encrypted store cannot be read before the first unlock. */
    data object Unavailable : WidgetSnapshot

    data object Failed : WidgetSnapshot

    data class Rows(val paused: Boolean, val rows: List<WidgetRow>) : WidgetSnapshot

    /** Privacy level Hidden; [count] is null when it could not be counted. */
    data class Count(val paused: Boolean, val count: Long?) : WidgetSnapshot

    companion object {
        /** Shown the moment history is cleared or a device is removed (W8). */
        val cleared: WidgetSnapshot = Rows(paused = false, rows = emptyList())
    }
}

/** Reads one widget instance's content through the runtime facade. */
internal class WidgetLoader(
    private val access: RuntimeAccess,
    private val nowMs: () -> Long = System::currentTimeMillis,
) {
    fun load(config: WidgetConfig, limit: Int): WidgetSnapshot {
        val paused = when (val reply = access.request("get_private_mode")) {
            is RuntimeReply.Ok -> reply.objectData()?.optBoolean("private_mode") == true
            is RuntimeReply.Failure -> return failure(reply)
        }
        if (config.privacy == WidgetPrivacy.Hidden) return WidgetSnapshot.Count(paused, count(config.filter))
        val rows = when (config.filter) {
            WidgetFilter.Snippets -> snippets(limit, config.privacy)
            WidgetFilter.History -> clips(pinnedOnly = false, limit, config.privacy)
            WidgetFilter.Pinned -> clips(pinnedOnly = true, limit, config.privacy)
        }
        return when (rows) {
            is Loaded.Rows -> WidgetSnapshot.Rows(paused, rows.rows)
            is Loaded.Failed -> failure(rows.reply)
        }
    }

    private sealed interface Loaded {
        class Rows(val rows: List<WidgetRow>) : Loaded
        class Failed(val reply: RuntimeReply) : Loaded
    }

    private fun clips(pinnedOnly: Boolean, limit: Int, privacy: WidgetPrivacy): Loaded {
        val params = JSONObject()
            .put("query", JSONObject().put("pinned_only", pinnedOnly))
            .put("limit", limit)
        val reply = access.request("history_query", params)
        if (reply !is RuntimeReply.Ok) return Loaded.Failed(reply)
        val items = reply.objectData()?.optJSONArray("items") ?: JSONArray()
        val now = nowMs()
        return Loaded.Rows(
            (0 until items.length()).mapNotNull { items.optJSONObject(it)?.let { item -> clipRow(item, privacy, now) } },
        )
    }

    private fun clipRow(item: JSONObject, privacy: WidgetPrivacy, now: Long): WidgetRow? {
        val id = item.optString("id").ifEmpty { return null }
        val presentation = ClipVisibility.of(item).presentationAt(now)
        if (presentation == ClipPresentation.Hidden) return null
        val masked = presentation == ClipPresentation.Masked
        val kind = clipKind(item)
        val text = when {
            masked || privacy != WidgetPrivacy.Full -> null
            kind == WidgetKind.Image || kind == WidgetKind.File ->
                item.optJSONObject("file_details")?.optString("filename")?.ifEmpty { null }
            else -> preview(item.optString("content"))
        }
        return WidgetRow(
            id = id,
            snippet = false,
            kind = kind,
            text = text,
            secondary = null,
            sourceApp = item.optString("source_app_name").takeUnless { item.isNull("source_app_name") || it.isEmpty() },
            createdAtMs = item.optLong("created_at"),
            pinned = item.optBoolean("pinned"),
            masked = masked,
            tap = if (masked) WidgetTap.AuthenticateAndCopy else WidgetTap.Copy,
        )
    }

    private fun snippets(limit: Int, privacy: WidgetPrivacy): Loaded {
        val reply = access.request("snippets", snippetList(limit))
        if (reply !is RuntimeReply.Ok) return Loaded.Failed(reply)
        val snippets = reply.objectData()?.optJSONArray("snippets") ?: JSONArray()
        return Loaded.Rows((0 until snippets.length()).mapNotNull { index ->
            val snippet = snippets.optJSONObject(index) ?: return@mapNotNull null
            val id = snippet.optString("id").ifEmpty { return@mapNotNull null }
            val concealed = snippet.optBoolean("concealed")
            WidgetRow(
                id = id,
                snippet = true,
                kind = WidgetKind.Snippet,
                text = preview(snippet.optString("title")),
                secondary = if (concealed || privacy != WidgetPrivacy.Full) null else preview(snippet.optString("preview")),
                sourceApp = null,
                createdAtMs = snippet.optLong("created_at"),
                pinned = snippet.optBoolean("pinned"),
                masked = concealed,
                tap = when {
                    concealed -> WidgetTap.AuthenticateAndCopy
                    snippet.optBoolean("template") -> WidgetTap.FillTemplate
                    else -> WidgetTap.Copy
                },
            )
        })
    }

    private fun count(filter: WidgetFilter): Long? = when (filter) {
        WidgetFilter.Snippets -> (access.request("snippets", snippetList(1)) as? RuntimeReply.Ok)
            ?.objectData()?.takeIf { it.has("total") }?.optLong("total")
        else -> {
            val ranges = JSONArray()
            if (filter == WidgetFilter.History) {
                ranges.put(JSONObject().put("from_ms", Long.MIN_VALUE).put("before_ms", Long.MAX_VALUE))
            }
            val params = JSONObject()
                .put("query", JSONObject().put("pinned_only", filter == WidgetFilter.Pinned))
                .put("ranges", ranges)
            (access.request("history_counts", params) as? RuntimeReply.Ok)?.objectData()?.let { counts ->
                val unpinned = counts.optJSONArray("ranges") ?: JSONArray()
                counts.optLong("pinned") + (0 until unpinned.length()).sumOf { unpinned.optLong(it) }
            }
        }
    }

    private fun snippetList(limit: Int): JSONObject = JSONObject().put(
        "op",
        JSONObject()
            .put("op", "list")
            .put("scope", JSONObject().put("kind", "all"))
            .put("cursor", JSONObject.NULL)
            .put("limit", limit),
    )

    private fun failure(reply: RuntimeReply): WidgetSnapshot = when ((reply as? RuntimeReply.Failure)?.code) {
        RuntimeErrorCode.NOT_READY,
        RuntimeErrorCode.KEY_LOCKED,
        RuntimeErrorCode.KEY_UNUSABLE -> WidgetSnapshot.Unavailable
        else -> WidgetSnapshot.Failed
    }

    companion object {
        /** Preview cap per row (W4): text in a widget leaves the encrypted store. */
        const val MAX_PREVIEW_CHARS = 120

        fun preview(text: String): String? {
            val collapsed = text.trim().replace(whitespace, " ")
            if (collapsed.isEmpty()) return null
            if (collapsed.codePointCount(0, collapsed.length) <= MAX_PREVIEW_CHARS) return collapsed
            return collapsed.substring(0, collapsed.offsetByCodePoints(0, MAX_PREVIEW_CHARS)).trimEnd() + "…"
        }

        private val whitespace = Regex("\\s+")

        private fun clipKind(item: JSONObject): WidgetKind = when (item.optString("content_class")) {
            "image" -> WidgetKind.Image
            "file", "other" -> WidgetKind.File
            else -> when (item.optString("semantic_kind")) {
                "link", "email" -> WidgetKind.Link
                "color" -> WidgetKind.Color
                "code", "json" -> WidgetKind.Code
                else -> WidgetKind.Text
            }
        }
    }
}
