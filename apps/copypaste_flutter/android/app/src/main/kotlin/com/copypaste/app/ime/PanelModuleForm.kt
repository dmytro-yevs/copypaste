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
import com.copypaste.app.ModuleForm
import com.copypaste.app.ModuleFormField
import com.copypaste.app.R

/**
 * A module's view form inside the panel. Text fields are typed on the panel
 * keypad, so nothing reaches the host field; choices and switches are chips.
 * With a [preview], each change asks for one after a short pause, never on a
 * timer.
 */
@SuppressLint("InflateParams")
internal class PanelModuleForm(
    private val context: Context,
    private val host: PanelTabHost,
    title: String,
    private val form: ModuleForm,
    private val concealed: Boolean,
    private val preview: ((values: Map<String, Any>, onPreview: (String?) -> Unit) -> Unit)?,
    private val onSubmit: (Map<String, Any>) -> Unit,
    private val onCancel: () -> Unit,
) {
    val view: View = LayoutInflater.from(context).inflate(R.layout.ime_module_form, null)
    private val fieldList: LinearLayout = view.findViewById(R.id.ime_form_fields)
    private val previewText: TextView = view.findViewById(R.id.ime_form_preview)
    private val submit: TextView = view.findViewById(R.id.ime_form_submit)
    private val values = form.initialValues()
    private val main = Handler(Looper.getMainLooper())
    private val runPreview = Runnable { requestPreview() }
    private var editing: String? = null
    private var generation = 0
    private var released = false

    init {
        view.findViewById<TextView>(R.id.ime_form_title).text = context.getString(R.string.ime_form_title, form.title ?: title)
        view.findViewById<View>(R.id.ime_form_cancel).apply {
            background = PanelStyle.control(context, filled = true)
            setOnClickListener {
                release()
                onCancel()
            }
        }
        submit.background = PanelStyle.control(context, filled = true, selected = true)
        form.submit.title.takeIf { it.isNotBlank() }?.let { submit.text = it }
        submit.setOnClickListener {
            if (!canSubmit()) {
                host.showNotice(context.getString(R.string.ime_form_missing))
                return@setOnClickListener
            }
            val submitted = values.toMap()
            release()
            onSubmit(submitted)
        }
        previewText.visibility = if (preview != null) View.VISIBLE else View.GONE
        render()
        if (preview != null) requestPreview()
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

    private fun text(id: String): String = values[id] as? String ?: ""

    private fun type(id: String, text: String) {
        values[id] = text(id) + text
        changed()
    }

    private fun canSubmit(): Boolean = form.fields.none { it.isMissing(values[it.id]) }

    private fun render() {
        fieldList.removeAllViews()
        for (field in form.fields) {
            when (field.kind) {
                ModuleFormField.Kind.BOOLEAN -> fieldList.addView(switch(field))
                ModuleFormField.Kind.CHOICES -> {
                    fieldList.addView(label(field))
                    fieldList.addView(choices(field))
                }
                ModuleFormField.Kind.TEXT, ModuleFormField.Kind.MULTILINE -> {
                    fieldList.addView(label(field))
                    fieldList.addView(input(field))
                    if (field.kind == ModuleFormField.Kind.MULTILINE && editing == field.id) {
                        fieldList.addView(
                            LinearLayout(context).apply {
                                addView(FeatureControls.chip(context, context.getString(R.string.ime_form_new_line)) { type(field.id, "\n") })
                            },
                        )
                    }
                }
                // The tab opens the app instead of a form it cannot fill in.
                ModuleFormField.Kind.UNSUPPORTED -> Unit
            }
            form.errors[field.id]?.let { fieldList.addView(error(it)) }
        }
        FeatureControls.setAvailable(submit, canSubmit())
    }

    private fun label(field: ModuleFormField): TextView = TextView(context).apply {
        text = if (field.required) context.getString(R.string.ime_form_required_label, field.title) else field.title
        setTextColor(context.getColor(R.color.app_color_muted_foreground))
        setTextSize(TypedValue.COMPLEX_UNIT_PX, context.resources.getDimension(R.dimen.app_text_size_x_small))
        setPadding(0, context.resources.getDimensionPixelSize(R.dimen.app_spacing_sm), 0, context.resources.getDimensionPixelSize(R.dimen.app_spacing_xxs))
        importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
    }

    private fun error(message: String): TextView = TextView(context).apply {
        text = message
        setTextColor(context.getColor(R.color.app_color_destructive))
        setTextSize(TypedValue.COMPLEX_UNIT_PX, context.resources.getDimension(R.dimen.app_text_size_x_small))
        setPadding(0, context.resources.getDimensionPixelSize(R.dimen.app_spacing_xxs), 0, 0)
        accessibilityLiveRegion = View.ACCESSIBILITY_LIVE_REGION_POLITE
    }

    private fun input(field: ModuleFormField): TextView = TextView(context).apply {
        val value = text(field.id)
        val shown = if (field.secret) SECRET_MASK.repeat(value.codePointCount(0, value.length)) else value
        val active = editing == field.id
        text = if (active) "$shown|" else shown
        hint = (field.default as? String)?.takeIf { !field.secret }
        isActivated = active
        maxLines = if (field.kind == ModuleFormField.Kind.MULTILINE) MULTILINE_ROWS else 1
        minHeight = context.resources.getDimensionPixelSize(R.dimen.app_control_size_large)
        gravity = android.view.Gravity.CENTER_VERTICAL
        val padding = context.resources.getDimensionPixelSize(R.dimen.app_spacing_md)
        setPadding(padding, 0, padding, 0)
        setTextColor(context.getColor(R.color.app_color_foreground))
        setHintTextColor(context.getColor(R.color.app_color_muted_foreground))
        setTextSize(TypedValue.COMPLEX_UNIT_PX, context.resources.getDimension(R.dimen.app_text_size_base))
        background = PanelStyle.input(context)
        contentDescription = listOfNotNull(
            field.title,
            value.takeIf { it.isNotEmpty() && !field.secret },
            context.getString(R.string.ime_form_required).takeIf { field.required },
        ).joinToString(", ")
        layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
        setOnClickListener { edit(field.id) }
    }

    /** Several options may be on at once, as in the app's module forms. */
    private fun choices(field: ModuleFormField): View = LinearLayout(context).apply {
        orientation = LinearLayout.HORIZONTAL
        val chosen = (values[field.id] as? List<*>).orEmpty().filterIsInstance<String>()
        for ((id, title) in field.options) {
            addView(
                FeatureControls.chip(context, title, selected = id in chosen) {
                    values[field.id] = if (id in chosen) chosen.filter { it != id } else chosen + id
                    changed()
                },
            )
        }
    }

    private fun switch(field: ModuleFormField): View = LinearLayout(context).apply {
        setPadding(0, context.resources.getDimensionPixelSize(R.dimen.app_spacing_sm), 0, 0)
        val on = values[field.id] == true
        addView(
            FeatureControls.chip(context, field.title, selected = on) {
                values[field.id] = !on
                changed()
            },
        )
    }

    private fun edit(id: String) {
        if (released) return
        editing = id
        host.openKeypad(
            KeypadTarget(
                onText = { text -> type(id, text) },
                onBackspace = {
                    val value = text(id)
                    if (value.isNotEmpty()) values[id] = value.substring(0, value.offsetByCodePoints(value.length, -1))
                    changed()
                },
                onClosed = {
                    if (editing == id) editing = null
                    if (!released) render()
                },
            ),
        )
        render()
    }

    private fun changed() {
        if (released) return
        render()
        if (preview == null) return
        main.removeCallbacks(runPreview)
        main.postDelayed(runPreview, PREVIEW_DEBOUNCE_MS)
    }

    private fun requestPreview() {
        val preview = preview ?: return
        val requested = ++generation
        preview(values.toMap()) { result ->
            if (requested != generation || released) return@preview
            previewText.text = when {
                result == null -> ""
                concealed || host.field.maskedMode -> context.getString(R.string.ime_form_preview_hidden)
                else -> result.trim()
            }
        }
    }

    private companion object {
        const val PREVIEW_DEBOUNCE_MS = 150L
        const val MULTILINE_ROWS = 3
        const val SECRET_MASK = "•"
    }
}
