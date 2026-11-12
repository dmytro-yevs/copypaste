package com.copypaste.app.ime

import android.content.Context
import android.icu.text.BreakIterator
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.BaseAdapter
import android.widget.FrameLayout
import android.widget.ListView
import com.copypaste.app.AndroidClipboardWriter
import com.copypaste.app.R
import com.copypaste.app.RuntimeAccessWorker
import com.copypaste.app.RuntimeErrorCode
import com.copypaste.app.RuntimeReply
import org.json.JSONObject

/** One snippet as the panel lists it, from an IPC `SnippetSummary` (and its search hit). */
internal data class PanelSnippet(
    val id: String,
    val title: String,
    val abbreviation: String?,
    val template: Boolean,
    val pinned: Boolean,
    val concealed: Boolean,
    val tier: String? = null,
    val duplicate: Boolean = false,
) {
    val name: String get() = title.ifBlank { abbreviation.orEmpty() }

    companion object {
        fun parse(summary: JSONObject?, hit: JSONObject? = null): PanelSnippet? {
            val id = summary?.optString("id")?.ifEmpty { null } ?: return null
            return PanelSnippet(
                id = id,
                title = summary.optString("title"),
                abbreviation = summary.nullableString("abbreviation"),
                template = summary.optBoolean("template", false),
                pinned = summary.optBoolean("pinned", false),
                concealed = summary.optBoolean("concealed", false),
                tier = hit?.nullableString("tier"),
                duplicate = hit?.optBoolean("duplicate", false) == true,
            )
        }
    }
}

/**
 * The Snippets tab: abbreviation-ranked search, template fill-in inside the
 * panel, and insertion by commit with the caret at the template's cursor.
 * In password and private fields a snippet is rendered, not "used", so its
 * local usage counters do not move (K26, K27).
 */
internal class PanelSnippetsTab(
    private val context: Context,
    private val host: PanelTabHost,
    private val worker: RuntimeAccessWorker,
    private val transforms: PanelTransforms,
    private val queue: PanelQueueTab,
) : PanelTabContent {
    override val tab = PanelTab.SNIPPETS
    private val rows = mutableListOf<PanelSnippet>()
    private val adapter = Adapter()
    private var root: View? = null
    private var state: FeatureState? = null
    private var formSlot: FrameLayout? = null
    private var form: PanelTemplateForm? = null
    private var query = ""
    private var cursor: String? = null
    private var loadingMore = false
    private var generation = 0
    /** A runtime parse cache exists only once this tab rendered something. */
    private var rendered = false

    override fun createView(parent: ViewGroup): View {
        val view = LayoutInflater.from(context).inflate(R.layout.ime_snippets_tab, parent, false)
        val list = view.findViewById<ListView>(R.id.ime_snippets_list)
        list.adapter = adapter
        list.setOnItemClickListener { _, _, position, _ -> rows.getOrNull(position)?.let(::insert) }
        list.setOnItemLongClickListener { _, _, position, _ ->
            rows.getOrNull(position)?.let { host.showActions(it.name, actionsFor(it)) }
            true
        }
        root = view
        state = FeatureState(view)
        formSlot = view.findViewById(R.id.ime_snippets_form_slot)
        form?.release()
        form = null
        return view
    }

    override fun onShown(query: String) = load(query)

    override fun onQueryChanged(query: String) = load(query)

    override fun release() {
        generation += 1
        form?.release()
        form = null
        rows.clear()
        cursor = null
        root = null
        state = null
        formSlot = null
        adapter.notifyDataSetChanged()
        if (rendered) {
            rendered = false
            worker.op("snippets", opOf("release")) {}
        }
    }

    fun actionsFor(snippet: PanelSnippet): List<PanelAction> = buildList {
        fun label(id: Int) = context.getString(id)
        add(PanelAction(label(R.string.ime_action_insert)) { insert(snippet) })
        add(PanelAction(label(R.string.ime_action_copy)) { unlocked(snippet) { fill(snippet) { copy(snippet, it) } } })
        add(PanelAction(label(R.string.ime_action_add_to_queue)) {
            unlocked(snippet) {
                fill(snippet) { values ->
                    queue.enqueue(JSONObject().put("kind", "snippet").put("id", snippet.id).put("values", JSONObject(values)))
                }
            }
        })
        if (host.field.allowsTransform) {
            add(PanelAction(label(R.string.ime_action_transform)) {
                unlocked(snippet) { fill(snippet) { values -> render(snippet, values, countUse = false) { text, _ -> transforms.openText(text, snippet.concealed) } } }
            })
        }
        add(PanelAction(label(if (snippet.pinned) R.string.ime_action_unpin else R.string.ime_action_pin)) {
            worker.op("snippets", opOf("set_pinned").put("id", snippet.id).put("pinned", !snippet.pinned)) { reload() }
        })
        add(PanelAction(label(R.string.ime_action_open_in_app)) { openApp(context, host) })
    }

    fun insert(snippet: PanelSnippet) = unlocked(snippet) {
        fill(snippet) { values -> render(snippet, values) { text, cursor -> commit(snippet, text, cursor) } }
    }

    private fun load(query: String) {
        val requested = ++generation
        this.query = query.trim()
        cursor = null
        loadingMore = false
        hideForm()
        // Earlier results stay visible until the new ones replace them.
        if (rows.isEmpty()) state?.show(R.drawable.ic_lucide_notebook_text, context.getString(R.string.ime_snippets_loading))
        request(requested, after = null)
    }

    private fun reload() = load(query)

    private fun loadMore() {
        val after = cursor ?: return
        if (loadingMore || query.isNotEmpty()) return
        loadingMore = true
        request(generation, after)
    }

    private fun request(requested: Int, after: String?) {
        val op = if (query.isEmpty()) {
            opOf("list").put("scope", JSONObject().put("kind", "all")).put("cursor", after ?: JSONObject.NULL).put("limit", PAGE_SIZE)
        } else {
            opOf("quick_search").put("query", query).put("include_snippets", true).put("limit", PAGE_SIZE)
        }
        worker.op("snippets", op) { reply ->
            if (requested != generation) return@op
            loadingMore = false
            if (reply !is RuntimeReply.Ok) {
                if (after == null) showFailed()
                return@op
            }
            val (page, next) = when (reply.kind) {
                "snippet_hits" -> reply.arrayData().objects().mapNotNull { PanelSnippet.parse(it.optJSONObject("snippet"), it) } to null
                else -> {
                    val data = reply.objectData()
                    data?.optJSONArray("snippets").objects().mapNotNull { PanelSnippet.parse(it) } to data?.nullableString("next_cursor")
                }
            }
            if (after == null) rows.clear()
            rows += page.filter { snippet -> rows.none { it.id == snippet.id } }
            cursor = next
            publish()
        }
    }

    private fun publish() {
        adapter.notifyDataSetChanged()
        val state = state ?: return
        when {
            rows.isNotEmpty() -> state.hide()
            query.isEmpty() -> state.show(
                R.drawable.ic_lucide_notebook_text,
                context.getString(R.string.ime_snippets_empty),
                context.getString(R.string.ime_snippets_create),
            ) { openApp(context, host) }
            else -> state.show(
                R.drawable.ic_lucide_search,
                context.getString(R.string.ime_snippets_no_results, query),
                context.getString(R.string.ime_snippets_create),
            ) { openApp(context, host) }
        }
    }

    private fun showFailed() {
        rows.clear()
        adapter.notifyDataSetChanged()
        state?.show(R.drawable.ic_lucide_notebook_text, context.getString(R.string.ime_snippets_error), context.getString(R.string.ime_retry)) {
            reload()
        }
    }

    /** A concealed snippet needs the device owner, like a secret clip. */
    private fun unlocked(snippet: PanelSnippet, action: () -> Unit) {
        if (snippet.concealed) host.unlockSecrets(action) else action()
    }

    /** Asks for the template's field values first when it has any. */
    private fun fill(snippet: PanelSnippet, then: (Map<String, String>) -> Unit) {
        if (!snippet.template) return then(emptyMap())
        worker.op("snippets", opOf("get").put("id", snippet.id)) { reply ->
            val detail = (reply as? RuntimeReply.Ok)?.objectData() ?: return@op failed(reply)
            val fields = detail.optJSONArray("fields").objects().mapNotNull(TemplateField::parse)
            if (fields.isEmpty()) then(emptyMap()) else showForm(snippet, fields, then)
        }
    }

    private fun showForm(snippet: PanelSnippet, fields: List<TemplateField>, then: (Map<String, String>) -> Unit) {
        val slot = formSlot ?: return
        hideForm()
        val created = PanelTemplateForm(
            context,
            host,
            snippet.name,
            fields,
            snippet.concealed,
            preview = { values, onPreview -> preview(snippet, values, onPreview) },
            onSubmit = { values ->
                hideForm()
                then(values)
            },
            onCancel = ::hideForm,
        )
        form = created
        slot.addView(created.view)
        slot.visibility = View.VISIBLE
    }

    private fun hideForm() {
        form?.release()
        form = null
        formSlot?.let {
            it.removeAllViews()
            it.visibility = View.GONE
        }
    }

    private fun preview(snippet: PanelSnippet, values: Map<String, String>, onPreview: (FormPreview?) -> Unit) {
        rendered = true
        val op = renderOp(snippet, values, purpose = "preview")
        worker.op("snippets", op) { reply ->
            val data = (reply as? RuntimeReply.Ok)?.objectData()
            onPreview(data?.let { FormPreview(it.optString("text"), it.optJSONArray("missing_required").strings()) })
        }
    }

    /**
     * The final text. Normal fields "use" the snippet (local usage counts);
     * password and private fields only render it.
     */
    private fun render(snippet: PanelSnippet, values: Map<String, String>, countUse: Boolean = host.field.recordsUsage, then: (String, Int) -> Unit) {
        rendered = true
        val op = if (countUse) {
            opOf("use").put("id", snippet.id).put("values", JSONObject(values)).put("clipboard", clipboardInput())
                .put("delivery", "return").put("locale", appLocale(context))
        } else {
            renderOp(snippet, values, purpose = "final")
        }
        worker.op("snippets", op) { reply ->
            val data = (reply as? RuntimeReply.Ok)?.objectData() ?: return@op failed(reply)
            if (data.optJSONArray("missing_required").strings().isNotEmpty()) {
                return@op host.showNotice(context.getString(R.string.ime_form_missing))
            }
            then(data.optString("text"), data.optInt("cursor_graphemes_from_end", 0))
        }
    }

    private fun renderOp(snippet: PanelSnippet, values: Map<String, String>, purpose: String): JSONObject =
        opOf("render").put("source", JSONObject().put("kind", "snippet").put("id", snippet.id))
            .put("values", JSONObject(values)).put("clipboard", clipboardInput()).put("purpose", purpose)
            .put("locale", appLocale(context))

    /** The panel may read the clipboard while it is the selected keyboard. */
    private fun clipboardInput(): JSONObject {
        val text = host.clipboardText()
        return if (text != null) JSONObject().put("kind", "provided").put("text", text) else JSONObject().put("kind", "host")
    }

    private fun commit(snippet: PanelSnippet, text: String, cursor: Int) {
        val outcome = host.insert.insertText(text, host.field)
        if (outcome == InsertOutcome.INSERTED && cursor > 0) host.insert.moveCaretBack(utf16FromEnd(text, cursor))
        host.afterInsert(outcome) { pasteViaClipboard(text, snippet.concealed) }
    }

    private fun copy(snippet: PanelSnippet, values: Map<String, String>) {
        if (host.field.recordsUsage) {
            val op = opOf("use").put("id", snippet.id).put("values", JSONObject(values)).put("clipboard", clipboardInput())
                .put("delivery", "copy").put("locale", appLocale(context))
            rendered = true
            worker.op("snippets", op) { reply ->
                if (reply is RuntimeReply.Ok) host.announce(context.getString(R.string.ime_copied)) else failed(reply)
            }
        } else {
            render(snippet, values) { text, _ ->
                if (write(text, snippet.concealed)) host.announce(context.getString(R.string.ime_copied))
            }
        }
    }

    /** The explicit fallback: our own clipboard write (never captured), then the editor's paste. */
    private fun pasteViaClipboard(text: String, secret: Boolean) {
        if (write(text, secret) && !host.insert.pasteFromClipboard()) {
            host.showNotice(context.getString(R.string.ime_insert_failed))
        }
    }

    private fun write(text: String, secret: Boolean): Boolean =
        AndroidClipboardWriter.writeText(context, text, "text/plain", secret = secret).also { written ->
            if (!written) host.showNotice(context.getString(R.string.ime_copy_failed))
        }

    private fun failed(reply: RuntimeReply) {
        val failure = reply as? RuntimeReply.Failure
        when (failure?.code) {
            RuntimeErrorCode.NOT_FOUND -> {
                host.showNotice(context.getString(R.string.ime_snippet_unavailable))
                reload()
            }
            RuntimeErrorCode.INVALID_REQUEST -> host.showNotice(context.getString(R.string.ime_form_missing))
            else -> host.showNotice(context.getString(R.string.ime_snippet_failed))
        }
    }

    private fun meta(snippet: PanelSnippet): String = listOfNotNull(
        snippet.abbreviation,
        tierLabel(snippet.tier)?.let(context::getString),
        context.getString(R.string.ime_snippet_template).takeIf { snippet.template },
        context.getString(R.string.ime_snippet_duplicate).takeIf { snippet.duplicate },
    ).joinToString(" · ")

    private fun description(snippet: PanelSnippet, meta: String): String = listOfNotNull(
        context.getString(if (snippet.concealed) R.string.ime_snippet_concealed else R.string.ime_snippet),
        snippet.name,
        meta.ifEmpty { null },
        context.getString(R.string.ime_pinned).takeIf { snippet.pinned },
    ).joinToString(", ")

    private inner class Adapter : BaseAdapter() {
        override fun getCount(): Int = rows.size
        override fun getItem(position: Int): PanelSnippet = rows[position]
        override fun getItemId(position: Int): Long = rows[position].id.hashCode().toLong()
        override fun hasStableIds(): Boolean = true

        override fun getView(position: Int, convertView: View?, parent: ViewGroup): View {
            val row = convertView ?: LayoutInflater.from(context).inflate(R.layout.ime_clip_row, parent, false)
            val snippet = rows[position]
            val meta = meta(snippet)
            val badge = when {
                snippet.concealed -> R.drawable.ic_lucide_lock
                snippet.pinned -> R.drawable.ic_lucide_pin
                else -> 0
            }
            bindFeatureRow(row, snippet.name, meta, badge, description(snippet, meta))
            bindRowActions(row, actionsFor(snippet))
            if (position >= rows.size - LOAD_MORE_THRESHOLD) loadMore()
            return row
        }
    }

    companion object {
        private const val PAGE_SIZE = 30
        private const val LOAD_MORE_THRESHOLD = 5

        fun tierLabel(tier: String?): Int? = when (tier) {
            "exact_abbreviation" -> R.string.ime_snippet_tier_exact
            "abbreviation_prefix" -> R.string.ime_snippet_tier_prefix
            "title" -> R.string.ime_snippet_tier_title
            "full_text" -> R.string.ime_snippet_tier_text
            else -> null
        }
    }
}

/** UTF-16 units taken by the last [graphemes] user-perceived characters of [text]. */
internal fun utf16FromEnd(text: String, graphemes: Int): Int {
    val breaks = BreakIterator.getCharacterInstance().apply { setText(text) }
    var position = text.length
    repeat(graphemes) {
        val previous = breaks.preceding(position)
        if (previous == BreakIterator.DONE) return text.length
        position = previous
    }
    return text.length - position
}
