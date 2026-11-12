package com.copypaste.app.ime

import android.annotation.SuppressLint
import android.content.Context
import android.content.res.ColorStateList
import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.RippleDrawable
import android.util.TypedValue
import android.view.LayoutInflater
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ListView
import android.widget.TextView
import com.copypaste.app.R
import kotlin.math.roundToInt

/** Token-based backgrounds shared by every panel control. */
internal object PanelStyle {
    fun control(context: Context, filled: Boolean, selected: Boolean = false): Drawable {
        val radius = context.resources.getDimension(R.dimen.app_radius_md)
        val shape = GradientDrawable().apply {
            cornerRadius = radius
            setColor(
                context.getColor(
                    when {
                        selected -> R.color.app_color_secondary
                        filled -> R.color.app_color_background
                        else -> android.R.color.transparent
                    },
                ),
            )
            if (selected || filled) {
                setStroke(context.resources.getDimensionPixelSize(R.dimen.app_spacing_xxs) / 2, context.getColor(R.color.app_color_border))
            }
        }
        val mask = GradientDrawable().apply {
            cornerRadius = radius
            setColor(context.getColor(R.color.app_color_foreground))
        }
        return RippleDrawable(ColorStateList.valueOf(context.getColor(R.color.app_color_accent)), shape, mask)
    }

    fun input(context: Context): Drawable = GradientDrawable().apply {
        cornerRadius = context.resources.getDimension(R.dimen.app_radius_md)
        setColor(context.getColor(R.color.app_color_input))
    }
}

/** User intents raised by [PanelView]. */
internal interface PanelViewEvents {
    fun onSearchTapped()
    fun onSwitchBack()
    fun onTabSelected(tab: PanelTab)
    fun onBackspace()
    fun onEnter()
    fun onUndo()
    fun onQueueIndicator()
    fun onItemTapped(item: PanelItem)
    fun onItemLongPressed(item: PanelItem)
    fun onResized(heightDp: Int)
}

/** The panel's framework views. It renders what the controller decides and holds no state of its own. */
@SuppressLint("InflateParams")
internal class PanelView(private val context: Context, private val events: PanelViewEvents) {
    val root: LinearLayout = LayoutInflater.from(context).inflate(R.layout.ime_panel, null) as LinearLayout
    private val search: TextView = root.findViewById(R.id.ime_search)
    private val tabs: LinearLayout = root.findViewById(R.id.ime_tabs)
    private val notice: View = root.findViewById(R.id.ime_notice)
    private val noticeText: TextView = root.findViewById(R.id.ime_notice_text)
    private val noticeAction: TextView = root.findViewById(R.id.ime_notice_action)
    val body: FrameLayout = root.findViewById(R.id.ime_body)
    val list: ListView = root.findViewById(R.id.ime_list)
    private val state: View = root.findViewById(R.id.ime_state)
    private val stateIcon: ImageView = root.findViewById(R.id.ime_state_icon)
    private val stateTitle: TextView = root.findViewById(R.id.ime_state_title)
    private val stateAction: TextView = root.findViewById(R.id.ime_state_action)
    private val actions: LinearLayout = root.findViewById(R.id.ime_actions)
    private val keypadSlot: FrameLayout = root.findViewById(R.id.ime_keypad_slot)
    private val undo: ImageButton = root.findViewById(R.id.ime_undo)
    private val queue: TextView = root.findViewById(R.id.ime_queue_indicator)
    private var tabContent: View? = null
    private val density = context.resources.displayMetrics.density
    /** Pixel bounds a resize drag may reach. */
    var bodyHeightRange: IntRange = 0..Int.MAX_VALUE

    init {
        search.background = PanelStyle.input(context)
        search.setOnClickListener { events.onSearchTapped() }
        listOf(R.id.ime_switch_back, R.id.ime_backspace, R.id.ime_undo, R.id.ime_enter).forEach {
            root.findViewById<View>(it).background = PanelStyle.control(context, filled = false)
        }
        root.findViewById<View>(R.id.ime_switch_back).setOnClickListener { events.onSwitchBack() }
        root.findViewById<View>(R.id.ime_backspace).apply {
            setOnClickListener { events.onBackspace() }
            repeatWhileHeld(this) { events.onBackspace() }
        }
        root.findViewById<View>(R.id.ime_enter).setOnClickListener { events.onEnter() }
        undo.setOnClickListener { events.onUndo() }
        queue.background = PanelStyle.control(context, filled = false)
        queue.setOnClickListener { events.onQueueIndicator() }
        noticeAction.background = PanelStyle.control(context, filled = true)
        stateAction.background = PanelStyle.control(context, filled = true)
        list.setOnItemClickListener { _, _, position, _ ->
            (list.adapter?.getItem(position) as? PanelItem)?.let(events::onItemTapped)
        }
        list.setOnItemLongClickListener { _, _, position, _ ->
            (list.adapter?.getItem(position) as? PanelItem)?.let(events::onItemLongPressed)
            true
        }
        bindResize(root.findViewById(R.id.ime_resize_handle))
        setUndoEnabled(false)
    }

    fun setBodyHeight(heightPx: Int) {
        body.layoutParams = body.layoutParams.apply { height = heightPx }
    }

    val bodyHeight: Int get() = body.layoutParams.height

    fun setTabs(available: List<PanelTab>, selected: PanelTab) {
        tabs.removeAllViews()
        tabs.visibility = if (available.size > 1) View.VISIBLE else View.GONE
        for (tab in available) {
            tabs.addView(
                TextView(context).apply {
                    text = context.getString(tabLabel(tab))
                    gravity = android.view.Gravity.CENTER
                    isSelected = tab == selected
                    setTextColor(context.getColor(if (tab == selected) R.color.app_color_foreground else R.color.app_color_muted_foreground))
                    setTextSize(TypedValue.COMPLEX_UNIT_PX, context.resources.getDimension(R.dimen.app_text_size_small))
                    background = PanelStyle.control(context, filled = false, selected = tab == selected)
                    minHeight = context.resources.getDimensionPixelSize(R.dimen.app_control_size_large)
                    layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
                    setOnClickListener { events.onTabSelected(tab) }
                },
            )
        }
    }

    fun setTabsVisible(visible: Boolean) {
        tabs.visibility = if (visible && tabs.childCount > 1) View.VISIBLE else View.GONE
    }

    fun setQuery(query: String, editing: Boolean) {
        search.text = if (editing) "$query|" else query
        search.isActivated = editing
    }

    /** Shows the history list, or one state view for loading, empty, error and locked. */
    fun setListState(listState: PanelListState) {
        hideTabContent()
        val ready = listState is PanelListState.Ready
        list.visibility = if (ready) View.VISIBLE else View.GONE
        state.visibility = if (ready) View.GONE else View.VISIBLE
        stateAction.visibility = View.GONE
        if (ready) return
        val (icon, title) = when (listState) {
            PanelListState.Loading -> R.drawable.ic_lucide_clipboard to context.getString(R.string.ime_loading)
            PanelListState.Locked -> R.drawable.ic_lucide_lock to context.getString(R.string.ime_locked)
            PanelListState.Failed -> R.drawable.ic_lucide_clipboard to context.getString(R.string.ime_error)
            is PanelListState.Empty -> when {
                listState.query.isNotEmpty() ->
                    R.drawable.ic_lucide_search to context.getString(R.string.ime_no_results, listState.query)
                listState.pinnedOnly -> R.drawable.ic_lucide_pin to context.getString(R.string.ime_empty_pinned)
                else -> R.drawable.ic_lucide_clipboard to context.getString(R.string.ime_empty_history)
            }
            is PanelListState.Ready -> return
        }
        stateIcon.setImageResource(icon)
        stateTitle.text = title
    }

    fun setStateAction(label: String?, action: (() -> Unit)?) {
        stateAction.text = label
        stateAction.visibility = if (label != null && action != null) View.VISIBLE else View.GONE
        stateAction.setOnClickListener { action?.invoke() }
    }

    /** Replaces the list with a tab body provided by another package. */
    fun showTabContent(view: View) {
        hideTabContent()
        list.visibility = View.GONE
        state.visibility = View.GONE
        tabContent = view
        body.addView(view, 0, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
    }

    fun hideTabContent() {
        tabContent?.let(body::removeView)
        tabContent = null
    }

    fun setNotice(text: String?, actionLabel: String? = null, action: (() -> Unit)? = null) {
        notice.visibility = if (text == null) View.GONE else View.VISIBLE
        noticeText.text = text
        noticeAction.text = actionLabel
        noticeAction.visibility = if (actionLabel != null && action != null) View.VISIBLE else View.GONE
        noticeAction.setOnClickListener { action?.invoke() }
    }

    fun setUndoEnabled(enabled: Boolean) {
        undo.isEnabled = enabled
        undo.alpha = if (enabled) 1f else DISABLED_ALPHA
    }

    fun setQueueCount(count: Int) {
        queue.visibility = if (count > 0) View.VISIBLE else View.INVISIBLE
        queue.text = context.resources.getQuantityString(R.plurals.ime_queue_count, count, count)
    }

    fun showKeypad(keypad: View?) {
        keypadSlot.removeAllViews()
        if (keypad != null) keypadSlot.addView(keypad)
        keypadSlot.visibility = if (keypad != null) View.VISIBLE else View.GONE
    }

    /** An in-panel sheet: an input method cannot host dialogs over the host app. */
    fun showActions(title: String, items: List<PanelAction>) {
        actions.removeAllViews()
        actions.addView(actionRow(title, header = true) {})
        for (item in items) actions.addView(actionRow(item.label) { hideActions(); item.run() })
        actions.addView(actionRow(context.getString(R.string.ime_action_cancel)) { hideActions() })
        actions.visibility = View.VISIBLE
    }

    fun hideActions() {
        actions.removeAllViews()
        actions.visibility = View.GONE
    }

    val actionsShown: Boolean get() = actions.visibility == View.VISIBLE

    @Suppress("DEPRECATION")
    fun announce(text: String) = root.announceForAccessibility(text)

    private fun actionRow(label: String, header: Boolean = false, onTap: () -> Unit): TextView = TextView(context).apply {
        text = label
        maxLines = if (header) 1 else 2
        ellipsize = android.text.TextUtils.TruncateAt.END
        gravity = android.view.Gravity.CENTER_VERTICAL
        minHeight = context.resources.getDimensionPixelSize(R.dimen.app_control_size_touch)
        val padding = context.resources.getDimensionPixelSize(R.dimen.app_spacing_lg)
        setPadding(padding, 0, padding, 0)
        setTextColor(context.getColor(if (header) R.color.app_color_muted_foreground else R.color.app_color_popover_foreground))
        setTextSize(
            TypedValue.COMPLEX_UNIT_PX,
            context.resources.getDimension(if (header) R.dimen.app_text_size_small else R.dimen.app_text_size_base),
        )
        if (!header) {
            background = PanelStyle.control(context, filled = false)
            setOnClickListener { onTap() }
        }
        layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
    }

    @SuppressLint("ClickableViewAccessibility")
    private fun bindResize(handle: View) {
        var startY = 0f
        var startHeight = 0
        handle.setOnTouchListener { _, event ->
            when (event.actionMasked) {
                MotionEvent.ACTION_DOWN -> {
                    startY = event.rawY
                    startHeight = body.height
                }
                MotionEvent.ACTION_MOVE ->
                    setBodyHeight((startHeight - (event.rawY - startY)).roundToInt().coerceIn(bodyHeightRange))
                MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> events.onResized((body.height / density).roundToInt())
            }
            true
        }
    }

    companion object {
        private const val DISABLED_ALPHA = 0.4f

        fun tabLabel(tab: PanelTab): Int = when (tab) {
            PanelTab.HISTORY -> R.string.ime_tab_history
            PanelTab.PINNED -> R.string.ime_tab_pinned
            PanelTab.SNIPPETS -> R.string.ime_tab_snippets
            PanelTab.QUEUE -> R.string.ime_tab_queue
        }
    }
}
