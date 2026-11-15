package com.copypaste.app.quickaccess

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.TypedValue
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.BaseAdapter
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ListView
import android.widget.TextView
import android.widget.Toast
import com.copypaste.app.R
import com.copypaste.app.RuntimeAccessWorker
import com.copypaste.app.RuntimeErrorCode
import com.copypaste.app.RuntimeReply
import com.copypaste.app.ime.PanelStyle
import java.util.Locale
import kotlin.math.roundToInt
import org.json.JSONArray
import org.json.JSONObject

/** The text a sheet transforms, or why there is none. */
internal data class PaletteSource(
    val text: String?,
    val editable: Boolean,
    val sensitive: Boolean,
    /** A state message instead of the palette: empty, not text or too large. */
    val problem: Int? = null,
) {
    companion object {
        /** The runtime refuses larger input; checking here keeps it off the binder. */
        const val MAX_INPUT_CHARS = 16 * 1024 * 1024

        fun of(text: CharSequence?, editable: Boolean, sensitive: Boolean, notText: Boolean = false): PaletteSource = when {
            notText -> PaletteSource(null, false, sensitive, R.string.quick_access_transform_not_text)
            text.isNullOrBlank() -> PaletteSource(null, false, sensitive, R.string.quick_access_transform_empty_source)
            text.length > MAX_INPUT_CHARS -> PaletteSource(null, false, sensitive, R.string.quick_access_transform_too_large)
            else -> PaletteSource(text.toString(), editable, sensitive)
        }
    }
}

/** What a [TransformSheet] needs from its activity. */
internal interface TransformSheetHost {
    /** Returns [text] to the field that sent the selection. */
    fun replace(text: String)
    fun dismiss()
    fun authenticate(done: (Boolean) -> Unit)
}

/**
 * The transform palette as framework views: pick a transform or pipeline,
 * see a bounded preview, then Replace, Copy or Save. Runtime calls run on
 * the shared worker; every reply is checked against the current selection.
 */
@SuppressLint("InflateParams")
internal class TransformSheet(
    private val context: Context,
    private val worker: RuntimeAccessWorker,
    private val source: PaletteSource,
    private val host: TransformSheetHost,
    private val locale: String = Locale.getDefault().toLanguageTag(),
) {
    val root: View = LayoutInflater.from(context).inflate(R.layout.quick_access_transform_sheet, null)
    private val sheet: LinearLayout = root.findViewById(R.id.qa_sheet)
    private val back: ImageButton = root.findViewById(R.id.qa_sheet_back)
    private val title: TextView = root.findViewById(R.id.qa_sheet_title)
    private val subtitle: TextView = root.findViewById(R.id.qa_sheet_subtitle)
    private val body: View = root.findViewById(R.id.qa_sheet_body)
    private val list: ListView = root.findViewById(R.id.qa_sheet_list)
    private val preview: View = root.findViewById(R.id.qa_sheet_preview)
    private val params: LinearLayout = root.findViewById(R.id.qa_sheet_params)
    private val output: TextView = root.findViewById(R.id.qa_sheet_output)
    private val status: TextView = root.findViewById(R.id.qa_sheet_status)
    private val state: View = root.findViewById(R.id.qa_sheet_state)
    private val stateIcon: ImageView = root.findViewById(R.id.qa_sheet_state_icon)
    private val stateTitle: TextView = root.findViewById(R.id.qa_sheet_state_title)
    private val stateAction: TextView = root.findViewById(R.id.qa_sheet_state_action)
    private val footer: LinearLayout = root.findViewById(R.id.qa_sheet_footer)
    private val main = Handler(Looper.getMainLooper())
    private val adapter = PaletteAdapter()
    private val actions = OutputAction.available(source.editable, source.sensitive)
    private val chosen = mutableMapOf<String, String>()
    private var selected: PaletteEntry? = null
    private var generation = 0
    private var previewReady = false
    private var noChanges = false
    private var outputSensitive = source.sensitive
    private var revealed = false
    private var busy = false
    private var shownOutput = ""
    private val runPreview = Runnable { preview() }

    init {
        sheet.background = GradientDrawable().apply {
            val radius = context.resources.getDimension(R.dimen.app_radius_xl)
            cornerRadii = floatArrayOf(radius, radius, radius, radius, 0f, 0f, 0f, 0f)
            setColor(context.getColor(R.color.app_color_popover))
        }
        output.background = PanelStyle.input(context)
        listOf(back, root.findViewById<View>(R.id.qa_sheet_close)).forEach {
            it.background = PanelStyle.control(context, filled = false)
        }
        stateAction.background = PanelStyle.control(context, filled = true)
        root.setOnClickListener { host.dismiss() }
        root.findViewById<View>(R.id.qa_sheet_close).setOnClickListener { host.dismiss() }
        back.setOnClickListener { showPalette() }
        output.setOnClickListener { if (maskedOutput) reveal() }
        list.adapter = adapter
        list.setOnItemClickListener { _, _, position, _ -> (adapter.getItem(position) as? PaletteEntry)?.let(::select) }
        val metrics = context.resources.displayMetrics
        body.layoutParams = body.layoutParams.apply { height = (metrics.heightPixels * BODY_SCREEN_SHARE).roundToInt() }
        subtitle.text = sourceSummary()
        renderFooter()
        val problem = source.problem
        if (problem != null) showState(R.drawable.ic_lucide_clipboard, problem) else load()
    }

    /** Drops lists and stale replies; the activity is going away. */
    fun release() {
        generation += 1
        main.removeCallbacks(runPreview)
        adapter.rows = emptyList()
    }

    /** Back closes the preview first, then the sheet. */
    fun onBack(): Boolean {
        if (selected == null) return false
        showPalette()
        return true
    }

    private val maskedOutput: Boolean get() = outputSensitive && !revealed

    private fun load() {
        showState(R.drawable.ic_lucide_wand_sparkles, R.string.quick_access_transform_loading)
        val request = generation
        worker.request("transforms", op("catalogue") { put("locale", locale) }) { catalogue ->
            if (request != generation) return@request
            val entries = (catalogue as? RuntimeReply.Ok)?.arrayData()
            if (entries == null) {
                showState(R.drawable.ic_lucide_wand_sparkles, R.string.quick_access_transform_error, R.string.quick_access_transform_retry) { load() }
                return@request
            }
            worker.request("transforms", op("preferences")) { preferences ->
                worker.request("transforms", op("pipelines")) { pipelines ->
                    if (request != generation) return@request
                    adapter.rows = TransformPalette.build(
                        entries,
                        (preferences as? RuntimeReply.Ok)?.objectData(),
                        (pipelines as? RuntimeReply.Ok)?.arrayData(),
                    ).flatMap { section -> listOfNotNull(section.title) + section.entries }
                    showPalette()
                }
            }
        }
    }

    private fun showPalette() {
        main.removeCallbacks(runPreview)
        generation += 1
        selected = null
        previewReady = false
        chosen.clear()
        back.visibility = View.GONE
        title.setText(R.string.quick_access_transform_title)
        subtitle.text = sourceSummary()
        preview.visibility = View.GONE
        if (adapter.rows.isEmpty()) {
            showState(R.drawable.ic_lucide_wand_sparkles, R.string.quick_access_transform_none)
        } else {
            state.visibility = View.GONE
            list.visibility = View.VISIBLE
        }
        renderFooter()
    }

    private fun select(entry: PaletteEntry) {
        selected = entry
        chosen.clear()
        back.visibility = View.VISIBLE
        title.text = entry.name
        subtitle.text = entry.description
        list.visibility = View.GONE
        state.visibility = View.GONE
        preview.visibility = View.VISIBLE
        renderParams(entry)
        preview()
    }

    private fun renderParams(entry: PaletteEntry) {
        params.removeAllViews()
        for (param in entry.params) {
            val value = chosen[param.key] ?: entry.valueOf(param)
            params.addView(
                chip(param.labelOf(value), selected = param.kind != PaletteParam.KIND_BOOL || value == "true") {
                    chosen[param.key] = param.next(value)
                    renderParams(entry)
                    main.removeCallbacks(runPreview)
                    main.postDelayed(runPreview, PREVIEW_DEBOUNCE_MS)
                },
            )
        }
        params.visibility = if (entry.params.isEmpty()) View.GONE else View.VISIBLE
    }

    private fun preview() {
        val entry = selected ?: return
        val text = source.text ?: return
        val request = ++generation
        previewReady = false
        setStatus(null)
        renderFooter()
        val payload = op("preview") {
            put("request_id", "quick-access-$request")
            put("source", textSource(previewPrefix(text)))
            put("steps", entry.steps(chosen))
        }
        worker.request("transforms", payload) { reply ->
            if (request != generation) return@request
            val result = (reply as? RuntimeReply.Ok)?.objectData()
            if (result == null) {
                showOutput("")
                setStatus(failureMessage(reply as? RuntimeReply.Failure))
                return@request
            }
            renderPreview(result, truncated = text.length > PREVIEW_CHARS)
        }
    }

    private fun renderPreview(result: JSONObject, truncated: Boolean) {
        val steps = result.optJSONArray("steps") ?: JSONArray()
        val failed = (0 until steps.length()).mapNotNull(steps::optJSONObject).firstOrNull { it.optJSONObject("error") != null }
        if (failed != null) {
            showOutput("")
            setStatusText(stepFailure(failed.getJSONObject("error")))
            return
        }
        val last = (0 until steps.length()).mapNotNull(steps::optJSONObject).lastOrNull { !it.optBoolean("not_run") }
        outputSensitive = source.sensitive || result.optBoolean("sensitive")
        noChanges = result.optBoolean("no_changes")
        previewReady = true
        showOutput(last?.optString("output_prefix") ?: source.text.orEmpty())
        val warned = (last?.optJSONArray("warnings")?.length() ?: 0) > 0
        setStatusText(
            listOfNotNull(
                R.string.quick_access_transform_no_changes.takeIf { noChanges },
                R.string.quick_access_transform_warning.takeIf { warned },
                R.string.quick_access_transform_truncated.takeIf { truncated || result.optBoolean("truncated") },
            ).joinToString(" · ") { context.getString(it) }.ifEmpty { null },
        )
        renderFooter()
    }

    private fun showOutput(text: String) {
        shownOutput = text
        output.text = if (maskedOutput) context.getString(R.string.quick_access_transform_sensitive) else text
        output.isClickable = maskedOutput
    }

    private fun reveal() = host.authenticate { success ->
        if (!success) return@authenticate
        revealed = true
        showOutput(shownOutput)
    }

    private fun renderFooter() {
        footer.removeAllViews()
        actions.forEachIndexed { index, action ->
            val enabled = previewReady && !busy && !(action == OutputAction.SAVE && noChanges)
            footer.addView(
                chip(context.getString(action.label), selected = index == 0, heightRes = R.dimen.app_control_size_touch) { run(action) }.apply {
                    isEnabled = enabled
                    alpha = if (enabled) 1f else DISABLED_ALPHA
                },
            )
        }
    }

    private fun run(action: OutputAction) {
        val entry = selected ?: return
        val text = source.text ?: return
        if (busy || !previewReady) return
        busy = true
        renderFooter()
        val copySensitive = action == OutputAction.COPY && outputSensitive
        val wire = when {
            action == OutputAction.REPLACE || copySensitive -> "return"
            action == OutputAction.COPY -> "copy"
            else -> "save_new_clip"
        }
        val payload = op("run") {
            put("source", textSource(text))
            put("steps_or_pipeline", entry.runSteps(chosen))
            put("action", wire)
            put("bypass_default", false)
        }
        worker.request("transforms", payload) { reply ->
            if (reply is RuntimeReply.Failure) return@request finishAction(failureMessage(reply))
            val result = (reply as RuntimeReply.Ok).objectData()?.takeIf { it.has("text") }?.optString("text")
            when {
                action == OutputAction.REPLACE -> replace(result)
                copySensitive -> copySecret(result)
                action == OutputAction.COPY -> done(copied = true)
                else -> {
                    toast(R.string.quick_access_transform_saved)
                    host.dismiss()
                }
            }
        }
    }

    private fun replace(result: String?) {
        if (result == null) return finishAction(R.string.quick_access_transform_action_failed)
        if (result.toByteArray(Charsets.UTF_8).size > MAX_REPLACE_BYTES) {
            return finishAction(R.string.quick_access_transform_replace_too_large)
        }
        host.replace(result)
    }

    /** The result of a sensitive source goes to the clipboard marked sensitive, never into history. */
    private fun copySecret(result: String?) {
        if (result == null) return finishAction(R.string.quick_access_transform_action_failed)
        val payload = op("deliver_text") {
            put("text", result)
            put("secret", true)
            put("delivery", "copy")
        }
        worker.request("clips", payload) { reply ->
            if (reply is RuntimeReply.Ok) done(copied = true) else finishAction(failureMessage(reply as? RuntimeReply.Failure))
        }
    }

    private fun done(copied: Boolean) {
        // Android 13+ confirms clipboard writes itself; a second toast would duplicate it.
        if (copied && Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) toast(R.string.quick_access_transform_copied)
        host.dismiss()
    }

    private fun finishAction(message: Int) {
        busy = false
        setStatus(message)
        renderFooter()
    }

    private fun failureMessage(failure: RuntimeReply.Failure?): Int = when (failure?.code) {
        RuntimeErrorCode.CONTENT_TOO_LARGE -> R.string.quick_access_transform_too_large
        RuntimeErrorCode.UNSUPPORTED_CONTENT -> R.string.quick_access_transform_not_text
        else -> R.string.quick_access_transform_failed
    }

    private fun stepFailure(error: JSONObject): String {
        val line = error.optInt("line", 0)
        val column = error.optInt("column", 0)
        return if (line > 0 && column > 0) {
            context.getString(R.string.quick_access_transform_failed_at, line, column)
        } else {
            context.getString(R.string.quick_access_transform_failed)
        }
    }

    private fun setStatus(message: Int?) = setStatusText(message?.let(context::getString))

    private fun setStatusText(message: String?) {
        status.text = message
        status.visibility = if (message == null) View.GONE else View.VISIBLE
    }

    private fun showState(icon: Int, message: Int, action: Int? = null, run: (() -> Unit)? = null) {
        list.visibility = View.GONE
        preview.visibility = View.GONE
        state.visibility = View.VISIBLE
        stateIcon.setImageResource(icon)
        stateTitle.setText(message)
        stateAction.visibility = if (action != null && run != null) View.VISIBLE else View.GONE
        if (action != null) stateAction.setText(action)
        stateAction.setOnClickListener { run?.invoke() }
    }

    private fun sourceSummary(): String? {
        val text = source.text ?: return null
        val chars = text.codePointCount(0, text.length)
        return context.resources.getQuantityString(R.plurals.ime_characters, chars, chars)
    }

    private fun toast(message: Int) = Toast.makeText(context.applicationContext, message, Toast.LENGTH_SHORT).show()

    private fun chip(label: String, selected: Boolean, heightRes: Int = R.dimen.app_control_size_regular, onTap: () -> Unit): TextView =
        TextView(context).apply {
            text = label
            gravity = android.view.Gravity.CENTER
            isSingleLine = true
            minHeight = context.resources.getDimensionPixelSize(heightRes)
            val padding = context.resources.getDimensionPixelSize(R.dimen.app_spacing_md)
            setPadding(padding, 0, padding, 0)
            setTextColor(context.getColor(R.color.app_color_foreground))
            setTextSize(TypedValue.COMPLEX_UNIT_PX, context.resources.getDimension(R.dimen.app_text_size_small))
            background = PanelStyle.control(context, filled = true, selected = selected)
            isSelected = selected
            layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT).apply {
                marginStart = context.resources.getDimensionPixelSize(R.dimen.app_spacing_sm)
            }
            setOnClickListener { onTap() }
        }

    /** At most [PREVIEW_CHARS], never ending inside a surrogate pair. */
    private fun previewPrefix(text: String): String {
        if (text.length <= PREVIEW_CHARS) return text
        val end = if (Character.isHighSurrogate(text[PREVIEW_CHARS - 1])) PREVIEW_CHARS - 1 else PREVIEW_CHARS
        return text.substring(0, end)
    }

    private fun textSource(text: String): JSONObject = JSONObject().put("kind", "text").put("text", text)

    /** Section titles (string resources) and entries in one list. */
    private inner class PaletteAdapter : BaseAdapter() {
        var rows: List<Any> = emptyList()
            set(value) {
                field = value
                notifyDataSetChanged()
            }

        override fun getCount(): Int = rows.size
        override fun getItem(position: Int): Any = rows[position]
        override fun getItemId(position: Int): Long = position.toLong()
        override fun getViewTypeCount(): Int = 2
        override fun getItemViewType(position: Int): Int = if (rows[position] is PaletteEntry) 0 else 1
        override fun isEnabled(position: Int): Boolean = rows[position] is PaletteEntry

        override fun getView(position: Int, convertView: View?, parent: ViewGroup): View {
            val row = rows[position]
            if (row !is PaletteEntry) {
                val header = convertView as? TextView ?: TextView(context).apply {
                    val padding = context.resources.getDimensionPixelSize(R.dimen.app_spacing_lg)
                    setPadding(padding, context.resources.getDimensionPixelSize(R.dimen.app_spacing_md), padding, 0)
                    setTextColor(context.getColor(R.color.app_color_muted_foreground))
                    setTextSize(TypedValue.COMPLEX_UNIT_PX, context.resources.getDimension(R.dimen.app_text_size_x_small))
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) isAccessibilityHeading = true
                }
                header.setText(row as Int)
                return header
            }
            val view = convertView ?: LayoutInflater.from(context).inflate(R.layout.quick_access_transform_row, parent, false)
                .apply { background = PanelStyle.control(context, filled = false) }
            view.findViewById<TextView>(R.id.qa_row_name).text = row.name
            view.findViewById<TextView>(R.id.qa_row_description).apply {
                text = row.description
                visibility = if (row.description.isEmpty()) View.GONE else View.VISIBLE
            }
            return view
        }
    }

    companion object {
        /** The runtime previews at most 64 KiB; sending more would only cross JNI for nothing. */
        const val PREVIEW_CHARS = 64 * 1024
        /** Results above this cannot travel back through the activity result binder. */
        const val MAX_REPLACE_BYTES = 1024 * 1024
        private const val PREVIEW_DEBOUNCE_MS = 120L
        private const val BODY_SCREEN_SHARE = 0.45
        private const val DISABLED_ALPHA = 0.4f
    }
}
