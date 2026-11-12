package com.copypaste.app.ime

import android.content.Context
import android.content.SharedPreferences
import android.os.Handler
import android.os.Looper
import android.text.format.Formatter
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.BaseAdapter
import android.widget.LinearLayout
import android.widget.ListView
import android.widget.TextView
import com.copypaste.app.AndroidClipboardWriter
import com.copypaste.app.R
import com.copypaste.app.RuntimeAccess
import com.copypaste.app.RuntimeAccessWorker
import com.copypaste.app.RuntimeErrorCode
import com.copypaste.app.RuntimeReply
import org.json.JSONArray
import org.json.JSONObject
import org.json.JSONTokener

/** One enumerable parameter of a transform; free-text parameters run only inside saved pipelines. */
internal data class TransformParam(
    val key: String,
    val label: String,
    val kind: String,
    val default: Any?,
    val options: List<Pair<String, String>>,
)

/** A transform or a saved pipeline the panel can run. */
internal data class TransformChoice(
    val pipeline: Boolean,
    val id: String,
    val name: String,
    val description: String,
    val category: String?,
    val version: Int,
    val keywords: List<String>,
    val params: List<TransformParam>,
    val freeText: Boolean,
    val steps: JSONArray? = null,
) {
    val key: String get() = (if (pipeline) "pipeline:" else "transform:") + id

    companion object {
        fun transform(info: JSONObject): TransformChoice? {
            val id = info.optString("id").ifEmpty { return null }
            return TransformChoice(
                pipeline = false,
                id = id,
                name = info.optString("name").ifEmpty { id },
                description = info.optString("description"),
                category = info.nullableString("category"),
                version = info.optInt("version", 1),
                keywords = info.optJSONArray("keywords").strings(),
                params = info.optJSONArray("params").objects().map { param ->
                    TransformParam(
                        key = param.optString("key"),
                        label = param.optString("label"),
                        kind = param.optString("kind"),
                        default = runCatching { JSONTokener(param.optString("default_json")).nextValue() }.getOrNull(),
                        options = param.optJSONArray("options").objects().map { it.optString("value") to it.optString("label") },
                    )
                },
                freeText = info.optJSONObject("flags")?.optBoolean("needs_free_text", false) == true,
            )
        }

        fun pipeline(pipeline: JSONObject): TransformChoice? {
            val id = pipeline.optString("id").ifEmpty { return null }
            if (pipeline.has("deleted_at") && !pipeline.isNull("deleted_at")) return null
            return TransformChoice(
                pipeline = true,
                id = id,
                name = pipeline.optString("name").ifEmpty { id },
                description = pipeline.nullableString("description").orEmpty(),
                category = null,
                version = 0,
                keywords = emptyList(),
                params = emptyList(),
                freeText = false,
                steps = pipeline.optJSONArray("steps") ?: JSONArray(),
            )
        }
    }
}

/** What the transforms read: everything a palette needs, loaded together off the main thread. */
internal data class TransformLibrary(
    val transforms: List<TransformChoice>,
    val pipelines: List<TransformChoice>,
    val favorites: List<String>,
    val hidden: Set<String>,
    val paramDefaults: Map<String, JSONObject>,
) {
    /** Transforms offered directly: not hidden and needing no free text (those run through pipelines). */
    val direct: List<TransformChoice> get() = transforms.filter { !it.freeText && it.id !in hidden }

    fun find(key: String): TransformChoice? = (direct + pipelines).firstOrNull { it.key == key }

    companion object {
        fun load(access: RuntimeAccess, locale: String): TransformLibrary? {
            fun call(op: JSONObject) = access.request("transforms", JSONObject().put("op", op)) as? RuntimeReply.Ok
            val catalogue = call(opOf("catalogue").put("locale", locale))?.arrayData() ?: return null
            val pipelines = call(opOf("pipelines"))?.arrayData()
            val preferences = call(opOf("preferences"))?.objectData()
            return TransformLibrary(
                transforms = catalogue.objects().mapNotNull(TransformChoice::transform),
                pipelines = pipelines.objects().mapNotNull(TransformChoice::pipeline),
                favorites = preferences?.optJSONArray("favorites").objects().map {
                    (if (it.optString("kind") == "pipeline") "pipeline:" else "transform:") + it.optString("id")
                },
                hidden = preferences?.optJSONArray("hidden_transforms").strings().toSet(),
                paramDefaults = preferences?.optJSONArray("param_defaults").objects().mapNotNull { entry ->
                    runCatching { entry.optString("transform_id") to JSONObject(entry.optString("params_json")) }.getOrNull()
                }.toMap(),
            )
        }
    }
}

/**
 * Transforms in the panel: the Transforms tab and the "Transform…" item
 * action. Sources are the field's selection, the clipboard, or a picked
 * history item or snippet output. Insert commits the result (replacing the
 * selection); Copy and Save go through the runtime. Nothing is kept once
 * the panel hides, and nothing is offered in password fields (K26).
 */
internal class PanelTransforms(
    private val context: Context,
    private val host: PanelTabHost,
    private val worker: RuntimeAccessWorker,
    private val preferences: SharedPreferences = context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE),
) : PanelTabContent {
    override val tab = PanelTab.TRANSFORMS

    private sealed interface Source {
        data class Selection(val text: String) : Source
        data object Clipboard : Source
        data class Clip(val id: String, val secret: Boolean) : Source
        data class Text(val text: String, val secret: Boolean) : Source
    }

    private sealed interface Row {
        data class Header(val label: String) : Row
        data class Choice(val choice: TransformChoice) : Row
    }

    private val main = Handler(Looper.getMainLooper())
    private val runPreview = Runnable { preview() }
    private val adapter = Adapter()
    private var views: Views? = null
    private var library: TransformLibrary? = null
    private var loading = false
    private var rows: List<Row> = emptyList()
    private var query = ""
    private var picked: Source? = null
    private var source: Source = Source.Clipboard
    private var selection: String? = null
    private var selected: TransformChoice? = null
    private var params = JSONObject()
    private var sensitive = false
    private var oversized = false
    private var session = 0
    private var previews = 0

    private class Views(root: View) {
        val sources: LinearLayout = root.findViewById(R.id.ime_transforms_sources)
        val list: ListView = root.findViewById(R.id.ime_transforms_list)
        val preview: View = root.findViewById(R.id.ime_transforms_preview)
        val params: LinearLayout = root.findViewById(R.id.ime_transforms_params)
        val output: TextView = root.findViewById(R.id.ime_transforms_output)
        val meta: TextView = root.findViewById(R.id.ime_transforms_meta)
        val actions: LinearLayout = root.findViewById(R.id.ime_transforms_actions)
        val state = FeatureState(root)
    }

    /** "Transform…" on a history text item; none in password fields (K26). */
    fun itemAction(item: PanelItem): PanelAction? {
        if (!host.field.allowsTransform || !item.isText) return null
        return PanelAction(context.getString(R.string.ime_action_transform)) {
            val pick = { open(Source.Clip(item.id, item.visibility.masked)) }
            if (item.visibility.masked) host.unlockSecrets(pick) else pick()
        }
    }

    /** Transforms rendered snippet output (Q-2). */
    fun openText(text: String, secret: Boolean) = open(Source.Text(text, secret))

    override fun createView(parent: ViewGroup): View {
        val root = LayoutInflater.from(context).inflate(R.layout.ime_transforms_tab, parent, false)
        val created = Views(root)
        created.list.adapter = adapter
        created.list.setOnItemClickListener { _, _, position, _ ->
            (rows.getOrNull(position) as? Row.Choice)?.let { select(it.choice) }
        }
        views = created
        return root
    }

    override fun onShown(query: String) {
        this.query = query.trim()
        val field = host.field
        selection = if (field.allowsTransform && !field.incognito) {
            host.inputConnection()?.getSelectedText(0)?.toString()?.takeIf { it.isNotEmpty() }
        } else {
            null
        }
        source = picked ?: selection?.let(Source::Selection) ?: Source.Clipboard
        render()
        if (field.allowsTransform && library == null) load()
    }

    override fun onQueryChanged(query: String) {
        this.query = query.trim()
        render()
    }

    override fun release() {
        session += 1
        main.removeCallbacks(runPreview)
        views = null
        library = null
        loading = false
        rows = emptyList()
        picked = null
        selection = null
        selected = null
        params = JSONObject()
        adapter.notifyDataSetChanged()
    }

    private fun open(source: Source) {
        picked = source
        selected = null
        host.selectTab(PanelTab.TRANSFORMS)
    }

    private fun load() {
        loading = true
        val requested = session
        val locale = appLocale(context)
        worker.perform({ access -> TransformLibrary.load(access, locale) }) { loaded ->
            if (requested != session) return@perform
            loading = false
            library = loaded
            render(failed = loaded == null)
        }
    }

    private fun render(failed: Boolean = false) {
        val views = views ?: return
        rows = if (host.field.allowsTransform) rowsFor(library) else emptyList()
        adapter.notifyDataSetChanged()
        renderSources(views.sources)
        when {
            !host.field.allowsTransform -> views.state.show(R.drawable.ic_lucide_lock, context.getString(R.string.ime_transforms_password))
            failed -> views.state.show(R.drawable.ic_lucide_wand_sparkles, context.getString(R.string.ime_transforms_error), context.getString(R.string.ime_retry)) { load() }
            loading || library == null -> views.state.show(R.drawable.ic_lucide_wand_sparkles, context.getString(R.string.ime_transforms_loading))
            rows.isEmpty() -> views.state.show(R.drawable.ic_lucide_search, context.getString(R.string.ime_transforms_no_matches))
            else -> views.state.hide()
        }
        renderSelection(views)
    }

    private fun rowsFor(library: TransformLibrary?): List<Row> {
        library ?: return emptyList()
        val rows = mutableListOf<Row>()
        fun section(label: String, choices: List<TransformChoice>) {
            if (choices.isEmpty()) return
            rows += Row.Header(label)
            choices.mapTo(rows, Row::Choice)
        }
        if (query.isNotEmpty()) {
            val needle = query.lowercase()
            section(
                context.getString(R.string.ime_transforms_results),
                (library.pipelines + library.direct).filter { choice ->
                    choice.name.lowercase().contains(needle) || choice.keywords.any { it.lowercase().contains(needle) }
                },
            )
            return rows
        }
        section(context.getString(R.string.ime_transforms_favorites), library.favorites.mapNotNull(library::find))
        section(context.getString(R.string.ime_transforms_recent), recents().mapNotNull(library::find))
        section(context.getString(R.string.ime_transforms_pipelines), library.pipelines)
        library.direct.groupBy { it.category }.forEach { (category, choices) ->
            section(context.getString(categoryLabel(category)), choices)
        }
        return rows
    }

    private fun renderSources(container: LinearLayout) {
        container.removeAllViews()
        if (!host.field.allowsTransform) return
        fun chip(label: Int, candidate: Source) = container.addView(
            FeatureControls.chip(context, context.getString(label), selected = source == candidate) { changeSource(candidate) },
        )
        selection?.let { chip(R.string.ime_transforms_source_selection, Source.Selection(it)) }
        chip(R.string.ime_transforms_source_clipboard, Source.Clipboard)
        when (val chosen = picked) {
            is Source.Clip -> chip(R.string.ime_transforms_source_history, chosen)
            is Source.Text -> chip(R.string.ime_transforms_source_snippet, chosen)
            else -> Unit
        }
        container.addView(
            FeatureControls.chip(context, context.getString(R.string.ime_transforms_pick_history)) {
                host.selectTab(PanelTab.HISTORY)
                host.showNotice(context.getString(R.string.ime_transforms_pick_hint))
            },
        )
    }

    private fun changeSource(candidate: Source) {
        source = candidate
        oversized = false
        render()
        schedulePreview()
    }

    private fun select(choice: TransformChoice) {
        selected = choice
        oversized = false
        params = JSONObject()
        choice.params.forEach { param -> param.default?.let { params.put(param.key, it) } }
        library?.paramDefaults?.get(choice.id)?.let { saved -> saved.keys().forEach { params.put(it, saved.get(it)) } }
        views?.output?.text = ""
        views?.meta?.text = ""
        render()
        schedulePreview()
    }

    private fun renderSelection(views: Views) {
        val choice = selected?.takeIf { host.field.allowsTransform }
        views.preview.visibility = if (choice != null) View.VISIBLE else View.GONE
        views.actions.visibility = views.preview.visibility
        views.params.removeAllViews()
        views.actions.removeAllViews()
        choice ?: return
        for (param in choice.params) {
            val current = params.opt(param.key)
            when {
                param.kind == "bool" -> views.params.addView(
                    FeatureControls.chip(context, param.label, selected = current == true) {
                        params.put(param.key, current != true)
                        changedParams()
                    },
                )
                param.kind == "choice" && param.options.isNotEmpty() -> {
                    val index = param.options.indexOfFirst { it.first == current?.toString() }
                    val label = param.options.getOrNull(index)?.second ?: param.options.first().second
                    views.params.addView(
                        FeatureControls.chip(context, context.getString(R.string.ime_transforms_param, param.label, label)) {
                            params.put(param.key, param.options[(index + 1) % param.options.size].first)
                            changedParams()
                        },
                    )
                }
            }
        }
        if (views.output.text.isNullOrEmpty()) views.output.text = context.getString(R.string.ime_transforms_previewing)
        val insert = FeatureControls.action(context, context.getString(R.string.ime_action_insert), weight = 2f) { execute(ACTION_RETURN) }
        insert.background = PanelStyle.control(context, filled = true, selected = true)
        FeatureControls.setAvailable(insert, !oversized)
        views.actions.addView(insert)
        views.actions.addView(FeatureControls.action(context, context.getString(R.string.ime_action_copy_short)) { execute(ACTION_COPY) })
        views.actions.addView(FeatureControls.action(context, context.getString(R.string.ime_action_save)) { execute(ACTION_SAVE) })
    }

    private fun changedParams() {
        oversized = false
        views?.let(::renderSelection)
        schedulePreview()
    }

    private fun schedulePreview() {
        main.removeCallbacks(runPreview)
        if (selected != null) main.postDelayed(runPreview, PREVIEW_DEBOUNCE_MS)
    }

    private fun preview() {
        val choice = selected ?: return
        val requested = ++previews
        val op = opOf("preview").put("request_id", "ime-$requested").put("source", sourceJson()).put("steps", stepsOf(choice))
        worker.op("transforms", op) { reply ->
            if (requested != previews || views == null) return@op
            val views = views ?: return@op
            val data = (reply as? RuntimeReply.Ok)?.objectData()
            if (data == null) {
                views.output.text = context.getString(failureMessage(reply))
                views.meta.text = ""
                return@op
            }
            sensitive = data.optBoolean("sensitive", false) || sourceSecret()
            val last = data.optJSONArray("steps").objects().lastOrNull()
            val error = last?.optJSONObject("error")
            views.output.text = when {
                error != null -> context.getString(R.string.ime_transforms_failed)
                sensitive -> context.getString(R.string.ime_transforms_sensitive)
                data.optBoolean("no_changes", false) -> context.getString(R.string.ime_transforms_no_changes)
                else -> last?.optString("output_prefix").orEmpty().take(PREVIEW_CHARS)
            }
            val counts = last?.optJSONObject("counts")
            views.meta.text = if (error == null && counts != null) {
                context.getString(R.string.ime_transforms_counts, counts.optLong("lines"), counts.optLong("words"), counts.optLong("chars"))
            } else {
                ""
            }
        }
    }

    private fun execute(action: String) {
        val choice = selected ?: return
        if (action == ACTION_RETURN && oversized) return offerCopyInstead()
        val steps = if (choice.pipeline) {
            JSONObject().put("kind", "pipeline").put("id", choice.id)
        } else {
            JSONObject().put("kind", "steps").put("steps", stepsOf(choice))
        }
        val op = opOf("run").put("source", sourceJson()).put("steps_or_pipeline", steps).put("action", action).put("bypass_default", false)
        worker.op("transforms", op) { reply ->
            if (reply !is RuntimeReply.Ok) return@op host.showNotice(context.getString(failureMessage(reply)))
            remember(choice)
            when (action) {
                ACTION_RETURN -> insertResult(reply.objectData()?.optString("text").orEmpty())
                ACTION_COPY -> host.announce(context.getString(R.string.ime_copied))
                else -> host.showNotice(context.getString(R.string.ime_transforms_saved))
            }
        }
    }

    /** Size guards: warn above 256 KiB, refuse above 1 MiB and offer Copy (binder limits). */
    private fun insertResult(text: String) {
        val bytes = utf8Length(text)
        when {
            bytes > MAX_INSERT_BYTES -> {
                oversized = true
                views?.let(::renderSelection)
                offerCopyInstead()
            }
            bytes > WARN_INSERT_BYTES -> host.showActions(
                context.getString(R.string.ime_transforms_large, Formatter.formatShortFileSize(context, bytes)),
                listOf(PanelAction(context.getString(R.string.ime_transforms_insert_anyway)) { commit(text) }),
            )
            else -> commit(text)
        }
    }

    private fun offerCopyInstead() = host.showNotice(
        context.getString(R.string.ime_transforms_too_large),
        context.getString(R.string.ime_action_copy),
    ) { execute(ACTION_COPY) }

    private fun commit(text: String) {
        val outcome = host.insert.insertText(text, host.field)
        host.afterInsert(outcome) {
            val written = AndroidClipboardWriter.writeText(context, text, "text/plain", secret = sensitive || sourceSecret())
            if (!written || !host.insert.pasteFromClipboard()) host.showNotice(context.getString(R.string.ime_insert_failed))
        }
    }

    private fun sourceSecret(): Boolean = when (val source = source) {
        is Source.Clip -> source.secret
        is Source.Text -> source.secret
        else -> false
    }

    private fun sourceJson(): JSONObject = when (val source = source) {
        is Source.Selection -> JSONObject().put("kind", "text").put("text", source.text)
        is Source.Text -> JSONObject().put("kind", "text").put("text", source.text)
        is Source.Clip -> JSONObject().put("kind", "clip").put("id", source.id)
        // The panel may read the clipboard while selected; a sensitive clip falls back to the runtime's newest item.
        Source.Clipboard -> host.clipboardText()?.let { JSONObject().put("kind", "text").put("text", it) }
            ?: JSONObject().put("kind", "clipboard")
    }

    private fun stepsOf(choice: TransformChoice): JSONArray = choice.steps ?: JSONArray().put(
        JSONObject().put("transform_id", choice.id).put("version", choice.version)
            .put("params_json", params.toString()).put("enabled", true),
    )

    private fun failureMessage(reply: RuntimeReply): Int = when ((reply as? RuntimeReply.Failure)?.code) {
        RuntimeErrorCode.UNSUPPORTED_CONTENT -> R.string.ime_transforms_not_text
        RuntimeErrorCode.NOT_FOUND -> R.string.ime_transforms_nothing
        RuntimeErrorCode.CONTENT_TOO_LARGE -> R.string.ime_transforms_too_large_input
        else -> R.string.ime_transforms_failed
    }

    private fun recents(): List<String> =
        runCatching { JSONArray(preferences.getString(RECENT, "[]")).strings() }.getOrDefault(emptyList())

    private fun remember(choice: TransformChoice) {
        val updated = (listOf(choice.key) + recents().filter { it != choice.key }).take(MAX_RECENTS)
        preferences.edit().putString(RECENT, JSONArray(updated).toString()).apply()
    }

    private inner class Adapter : BaseAdapter() {
        override fun getCount(): Int = if (views == null) 0 else rows.size
        override fun getItem(position: Int): Any = rows[position]
        override fun getItemId(position: Int): Long = position.toLong()
        override fun getViewTypeCount(): Int = 2
        override fun getItemViewType(position: Int): Int = if (rows[position] is Row.Header) 0 else 1
        override fun isEnabled(position: Int): Boolean = rows[position] is Row.Choice

        override fun getView(position: Int, convertView: View?, parent: ViewGroup): View = when (val row = rows[position]) {
            is Row.Header -> FeatureControls.sectionHeader(context, row.label)
            is Row.Choice -> {
                val view = convertView ?: LayoutInflater.from(context).inflate(R.layout.ime_clip_row, parent, false)
                val choice = row.choice
                val chosen = choice.key == selected?.key
                bindFeatureRow(
                    view,
                    choice.name,
                    choice.description,
                    if (chosen) R.drawable.ic_lucide_wand_sparkles else 0,
                    listOf(choice.name, choice.description).filter(String::isNotEmpty).joinToString(", "),
                )
                view.isSelected = chosen
                view
            }
        }
    }

    companion object {
        const val PREFERENCES = "quick-access-transforms"
        private const val RECENT = "recent"
        private const val MAX_RECENTS = 5
        private const val PREVIEW_DEBOUNCE_MS = 120L
        private const val PREVIEW_CHARS = 2_000
        const val WARN_INSERT_BYTES = 256 * 1024
        const val MAX_INSERT_BYTES = 1024 * 1024
        private const val ACTION_RETURN = "return"
        private const val ACTION_COPY = "copy"
        private const val ACTION_SAVE = "save_new_clip"

        fun categoryLabel(category: String?): Int = when (category) {
            "structure" -> R.string.ime_transforms_category_structure
            "encode" -> R.string.ime_transforms_category_encode
            "case" -> R.string.ime_transforms_category_case
            "hash" -> R.string.ime_transforms_category_hash
            "lines" -> R.string.ime_transforms_category_lines
            "clean" -> R.string.ime_transforms_category_clean
            "url" -> R.string.ime_transforms_category_url
            "markup" -> R.string.ime_transforms_category_markup
            "dev" -> R.string.ime_transforms_category_dev
            else -> R.string.ime_transforms_category_other
        }

        /** UTF-8 size without encoding a copy of a possibly multi-megabyte result. */
        fun utf8Length(text: CharSequence): Int {
            var bytes = 0
            var index = 0
            while (index < text.length) {
                val char = text[index]
                bytes += when {
                    char.code < 0x80 -> 1
                    char.code < 0x800 -> 2
                    Character.isHighSurrogate(char) && index + 1 < text.length && Character.isLowSurrogate(text[index + 1]) -> {
                        index += 1
                        4
                    }
                    else -> 3
                }
                index += 1
            }
            return bytes
        }
    }
}
