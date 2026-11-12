package com.copypaste.app.ime

import android.annotation.SuppressLint
import android.content.Context
import android.os.Handler
import android.os.Looper
import android.util.TypedValue
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.LinearLayout
import android.widget.TextView
import com.copypaste.app.R
import org.json.JSONObject

/** One user-input placeholder of a template, read from an IPC `TemplateField`. */
internal data class TemplateField(
    val name: String,
    val label: String,
    val kind: Kind,
    val default: String?,
    val options: List<String>,
    val required: Boolean,
) {
    enum class Kind { TEXT, MULTILINE, CHOICE }

    /** The value a form starts with: the default, or a choice's first option. */
    val initial: String get() = default ?: options.firstOrNull().takeIf { kind == Kind.CHOICE }.orEmpty()

    companion object {
        fun parse(field: JSONObject): TemplateField? {
            val name = field.optString("name").ifEmpty { return null }
            val options = field.optJSONArray("options")
            return TemplateField(
                name = name,
                label = field.optString("label").ifEmpty { name },
                kind = when (field.optString("kind")) {
                    "multiline" -> Kind.MULTILINE
                    "choice" -> Kind.CHOICE
                    else -> Kind.TEXT
                },
                default = field.nullableString("default"),
                options = if (options == null) emptyList() else (0 until options.length()).map(options::optString),
                required = field.optBoolean("required", false),
            )
        }
    }
}

/** What a preview render reported back to the form. */
internal data class FormPreview(val text: String?, val missingRequired: List<String>)

/**
 * The fill-in form of a template inside the panel. Text fields are typed on
 * the panel keypad, so nothing reaches the host field; choices are chips.
 * Each change asks for a preview after a short pause, never on a timer.
 */
@SuppressLint("InflateParams")
internal class PanelTemplateForm(
    private val context: Context,
    private val host: PanelTabHost,
    title: String,
    private val fields: List<TemplateField>,
    private val concealed: Boolean,
    private val preview: (values: Map<String, String>, onPreview: (FormPreview?) -> Unit) -> Unit,
    private val onSubmit: (Map<String, String>) -> Unit,
    private val onCancel: () -> Unit,
) {
    val view: View = LayoutInflater.from(context).inflate(R.layout.ime_snippets_form, null)
    private val fieldList: LinearLayout = view.findViewById(R.id.ime_form_fields)
    private val previewText: TextView = view.findViewById(R.id.ime_form_preview)
    private val submit: TextView = view.findViewById(R.id.ime_form_insert)
    private val values = linkedMapOf<String, String>().apply { fields.forEach { put(it.name, it.initial) } }
    private val main = Handler(Looper.getMainLooper())
    private val runPreview = Runnable { requestPreview() }
    private var editing: String? = null
    private var generation = 0
    private var missing: List<String> = emptyList()
    private var released = false

    init {
        view.findViewById<TextView>(R.id.ime_form_title).text = context.getString(R.string.ime_form_title, title)
        view.findViewById<View>(R.id.ime_form_cancel).apply {
            background = PanelStyle.control(context, filled = true)
            setOnClickListener {
                release()
                onCancel()
            }
        }
        submit.background = PanelStyle.control(context, filled = true, selected = true)
        submit.setOnClickListener {
            if (!canSubmit()) {
                host.showNotice(context.getString(R.string.ime_form_missing))
                return@setOnClickListener
            }
            val submitted = values.toMap()
            release()
            onSubmit(submitted)
        }
        render()
        requestPreview()
    }

    /** Stops editing and drops pending previews; the form is not used again. */
    fun release() {
        if (released) return
        released = true
        generation += 1
        main.removeCallbacks(runPreview)
        if (editing != null) host.closeKeypad()
        editing = null
    }

    private fun type(name: String, text: String) {
        values[name] = values[name].orEmpty() + text
        changed()
    }

    private fun canSubmit(): Boolean =
        missing.isEmpty() && fields.none { it.required && values[it.name].isNullOrBlank() }

    private fun render() {
        fieldList.removeAllViews()
        for (field in fields) {
            fieldList.addView(label(field))
            when (field.kind) {
                TemplateField.Kind.CHOICE -> fieldList.addView(choices(field))
                else -> {
                    fieldList.addView(input(field))
                    if (field.kind == TemplateField.Kind.MULTILINE && editing == field.name) {
                        fieldList.addView(
                            LinearLayout(context).apply {
                                addView(FeatureControls.chip(context, context.getString(R.string.ime_form_new_line)) { type(field.name, "\n") })
                            },
                        )
                    }
                }
            }
        }
        FeatureControls.setAvailable(submit, canSubmit())
    }

    private fun label(field: TemplateField): TextView = TextView(context).apply {
        text = if (field.required) context.getString(R.string.ime_form_required_label, field.label) else field.label
        setTextColor(context.getColor(R.color.app_color_muted_foreground))
        setTextSize(TypedValue.COMPLEX_UNIT_PX, context.resources.getDimension(R.dimen.app_text_size_x_small))
        setPadding(0, context.resources.getDimensionPixelSize(R.dimen.app_spacing_sm), 0, context.resources.getDimensionPixelSize(R.dimen.app_spacing_xxs))
        importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
    }

    private fun input(field: TemplateField): TextView = TextView(context).apply {
        val value = values[field.name].orEmpty()
        val active = editing == field.name
        text = if (active) "$value|" else value
        hint = field.default
        isActivated = active
        maxLines = if (field.kind == TemplateField.Kind.MULTILINE) MULTILINE_ROWS else 1
        minHeight = context.resources.getDimensionPixelSize(R.dimen.app_control_size_large)
        gravity = android.view.Gravity.CENTER_VERTICAL
        val padding = context.resources.getDimensionPixelSize(R.dimen.app_spacing_md)
        setPadding(padding, 0, padding, 0)
        setTextColor(context.getColor(R.color.app_color_foreground))
        setHintTextColor(context.getColor(R.color.app_color_muted_foreground))
        setTextSize(TypedValue.COMPLEX_UNIT_PX, context.resources.getDimension(R.dimen.app_text_size_base))
        background = PanelStyle.input(context)
        contentDescription = listOfNotNull(
            field.label,
            value.ifEmpty { null },
            context.getString(R.string.ime_form_required).takeIf { field.required },
        ).joinToString(", ")
        layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
        setOnClickListener { edit(field.name) }
    }

    private fun choices(field: TemplateField): View = LinearLayout(context).apply {
        orientation = LinearLayout.HORIZONTAL
        for (option in field.options) {
            addView(
                FeatureControls.chip(context, option, selected = values[field.name] == option) {
                    values[field.name] = option
                    changed()
                },
            )
        }
    }

    private fun edit(name: String) {
        if (released) return
        editing = name
        host.openKeypad(
            KeypadTarget(
                onText = { text -> type(name, text) },
                onBackspace = {
                    val value = values[name].orEmpty()
                    if (value.isNotEmpty()) values[name] = value.substring(0, value.offsetByCodePoints(value.length, -1))
                    changed()
                },
                onClosed = {
                    if (editing == name) editing = null
                    if (!released) render()
                },
            ),
        )
        render()
    }

    private fun changed() {
        if (released) return
        render()
        main.removeCallbacks(runPreview)
        main.postDelayed(runPreview, PREVIEW_DEBOUNCE_MS)
    }

    private fun requestPreview() {
        val requested = ++generation
        preview(values.toMap()) { result ->
            if (requested != generation || released) return@preview
            missing = result?.missingRequired.orEmpty()
            previewText.text = when {
                result == null -> ""
                concealed -> context.getString(R.string.ime_form_preview_hidden)
                else -> result.text.orEmpty().trim()
            }
            render()
        }
    }

    private companion object {
        const val PREVIEW_DEBOUNCE_MS = 150L
        const val MULTILINE_ROWS = 3
    }
}
