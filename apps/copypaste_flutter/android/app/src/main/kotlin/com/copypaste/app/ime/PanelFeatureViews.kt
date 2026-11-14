package com.copypaste.app.ime

import android.content.Context
import android.content.Intent
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import com.copypaste.app.R
import com.copypaste.app.RuntimeAccessWorker
import com.copypaste.app.RuntimeReply
import org.json.JSONArray
import org.json.JSONObject

/** Wires the snippets, queue and transforms features into the panel. */
internal fun registerPanelFeatures(context: Context, panel: PanelController, worker: RuntimeAccessWorker) {
    val transforms = PanelTransforms(context, panel, worker)
    val queue = PanelQueueTab(context, panel, worker)
    panel.registerTab(PanelSnippetsTab(context, panel, worker, transforms, queue))
    panel.registerTab(queue)
    panel.registerTab(transforms)
    panel.registerInsertDefault(transforms)
    panel.registerItemAction(transforms::originalAction)
    panel.registerItemAction(transforms::itemAction)
    panel.registerItemAction(queue::itemAction)
}

/** One grouped runtime method, such as `snippets` or `paste_queue`, with its op. */
internal fun RuntimeAccessWorker.op(method: String, op: JSONObject, onReply: (RuntimeReply) -> Unit) =
    request(method, JSONObject().put("op", op), onReply)

internal fun opOf(name: String): JSONObject = JSONObject().put("op", name)

/** The app's BCP 47 locale, for template dates and transform names. */
internal fun appLocale(context: Context): String = context.resources.configuration.locales[0].toLanguageTag()

/** The CopyPaste app, where snippets and pipelines are edited. */
internal fun openApp(context: Context, host: PanelTabHost) {
    val launch = context.packageManager.getLaunchIntentForPackage(context.packageName) ?: return
    host.startActivity(launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
}

internal fun JSONObject.nullableString(key: String): String? =
    if (has(key) && !isNull(key)) optString(key).ifEmpty { null } else null

internal fun JSONArray?.objects(): List<JSONObject> =
    if (this == null) emptyList() else (0 until length()).mapNotNull(::optJSONObject)

internal fun JSONArray?.strings(): List<String> =
    if (this == null) emptyList() else (0 until length()).map(::optString)

/** The one loading, empty, error and unavailable state of the feature tabs. */
internal class FeatureState(root: View) {
    private val view: View = root.findViewById(R.id.ime_state)
    private val icon: ImageView = root.findViewById(R.id.ime_state_icon)
    private val title: TextView = root.findViewById(R.id.ime_state_title)
    private val action: TextView = root.findViewById(R.id.ime_state_action)

    init {
        action.background = PanelStyle.control(root.context, filled = true)
    }

    fun show(iconRes: Int, text: String, actionLabel: String? = null, onAction: (() -> Unit)? = null) {
        view.visibility = View.VISIBLE
        icon.setImageResource(iconRes)
        title.text = text
        action.text = actionLabel
        action.visibility = if (actionLabel != null && onAction != null) View.VISIBLE else View.GONE
        action.setOnClickListener { onAction?.invoke() }
    }

    fun hide() {
        view.visibility = View.GONE
    }
}

/** Token-styled controls shared by the feature tabs. */
internal object FeatureControls {
    /** A compact toggle or choice chip; [selected] marks it as on. */
    fun chip(context: Context, label: String, selected: Boolean = false, onTap: () -> Unit): TextView =
        control(context, label, filled = !selected, selected = selected, onTap = onTap).apply {
            minHeight = context.resources.getDimensionPixelSize(R.dimen.app_control_size_compact)
            val horizontal = context.resources.getDimensionPixelSize(R.dimen.app_spacing_md)
            setPadding(horizontal, 0, horizontal, 0)
            isSelected = selected
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { marginEnd = context.resources.getDimensionPixelSize(R.dimen.app_spacing_xs) }
        }

    /** An action of a tab's fixed action row; all share one height. */
    fun action(context: Context, label: String, weight: Float = 1f, onTap: () -> Unit): TextView =
        control(context, label, filled = true, selected = false, onTap = onTap).apply {
            minHeight = context.resources.getDimensionPixelSize(R.dimen.app_control_size_large)
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, weight).apply {
                val margin = context.resources.getDimensionPixelSize(R.dimen.app_spacing_xxs)
                setMargins(margin, 0, margin, 0)
            }
        }

    /**
     * Dims a control that cannot act now. It stays tappable so the tap can
     * say why (an inline reason, never a silent no-op).
     */
    fun setAvailable(view: View, available: Boolean) {
        view.alpha = if (available) 1f else DISABLED_ALPHA
    }

    fun sectionHeader(context: Context, label: String): TextView = TextView(context).apply {
        text = label
        setTextColor(context.getColor(R.color.app_color_muted_foreground))
        setTextSize(TypedValue.COMPLEX_UNIT_PX, context.resources.getDimension(R.dimen.app_text_size_x_small))
        val horizontal = context.resources.getDimensionPixelSize(R.dimen.app_spacing_md)
        setPadding(horizontal, context.resources.getDimensionPixelSize(R.dimen.app_spacing_sm), horizontal, 0)
        isFocusable = false
        accessibilityHeading()
    }

    private fun TextView.accessibilityHeading() {
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.P) isAccessibilityHeading = true
    }

    private fun control(context: Context, label: String, filled: Boolean, selected: Boolean, onTap: () -> Unit): TextView =
        TextView(context).apply {
            text = label
            gravity = Gravity.CENTER
            maxLines = 1
            ellipsize = android.text.TextUtils.TruncateAt.END
            setTextColor(context.getColor(R.color.app_color_foreground))
            setTextSize(TypedValue.COMPLEX_UNIT_PX, context.resources.getDimension(R.dimen.app_text_size_small))
            background = PanelStyle.control(context, filled = filled, selected = selected)
            setOnClickListener { onTap() }
        }

    private const val DISABLED_ALPHA = 0.4f
}

/** Binds one `ime_clip_row` for a feature list. [badge] 0 hides the badge. */
internal fun bindFeatureRow(row: View, text: String, meta: String, badge: Int, description: String, usable: Boolean = true) {
    row.findViewById<TextView>(R.id.ime_row_text).text = text
    row.findViewById<TextView>(R.id.ime_row_meta).apply {
        this.text = meta
        visibility = if (meta.isEmpty()) View.GONE else View.VISIBLE
    }
    row.findViewById<ImageView>(R.id.ime_row_thumbnail).visibility = View.GONE
    row.findViewById<ImageView>(R.id.ime_row_badge).apply {
        if (badge != 0) setImageResource(badge) else setImageDrawable(null)
        visibility = if (badge != 0) View.VISIBLE else View.GONE
    }
    row.alpha = if (usable) 1f else 0.5f
    row.contentDescription = description
}

/** Replaces [row]'s TalkBack custom actions with [actions] (the long-press sheet's). */
internal fun bindRowActions(row: View, actions: List<PanelAction>) {
    (row.getTag(R.id.ime_row_badge) as? List<*>)?.forEach { id ->
        if (id is Int) androidx.core.view.ViewCompat.removeAccessibilityAction(row, id)
    }
    val ids = actions.map { action ->
        androidx.core.view.ViewCompat.addAccessibilityAction(row, action.label) { _, _ -> action.run(); true }
    }
    row.setTag(R.id.ime_row_badge, ids)
}
