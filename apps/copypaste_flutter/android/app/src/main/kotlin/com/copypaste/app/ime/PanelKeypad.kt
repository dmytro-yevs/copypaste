package com.copypaste.app.ime

import android.annotation.SuppressLint
import android.content.Context
import android.content.res.Resources
import android.os.Handler
import android.os.Looper
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import com.copypaste.app.R
import java.util.Locale

/** Letter rows of one keypad layout; the symbol page is shared. */
internal enum class KeypadLayout(val key: String, val label: String, val rows: List<String>) {
    LATIN("latin", "ABC", listOf("qwertyuiop", "asdfghjkl", "zxcvbnm")),
    UKRAINIAN("uk", "УКР", listOf("йцукенгшщзхї", "фівапролджє", "ячсмитьбюґ")),
    CYRILLIC("cyrillic", "АБВ", listOf("йцукенгшщзхъ", "фывапролджэ", "ячсмитьбюё"));

    companion object {
        val SYMBOLS = listOf("1234567890", "@#&-_.,:/'", "+=*!?()%\"")
        private val cyrillicLanguages = setOf("ru", "be", "bg", "sr", "mk", "kk", "ky", "mn", "tg")

        /** Latin plus the Cyrillic layouts the device's enabled locales ask for. */
        fun available(locales: List<Locale>): List<KeypadLayout> {
            val languages = locales.map { it.language }
            return buildList {
                add(LATIN)
                if ("uk" in languages) add(UKRAINIAN)
                if (languages.any { it in cyrillicLanguages }) add(CYRILLIC)
            }
        }

        fun deviceLocales(): List<Locale> {
            val list = Resources.getSystem().configuration.locales
            return (0 until list.size()).map(list::get)
        }
    }
}

/**
 * The compact keypad for the panel's own search and form fields. Its keys
 * edit only panel text; nothing typed here reaches the host field.
 */
@SuppressLint("ViewConstructor")
internal class PanelKeypad(
    context: Context,
    private val layouts: List<KeypadLayout>,
    initial: String?,
    private val onText: (String) -> Unit,
    private val onBackspace: () -> Unit,
    private val onDone: () -> Unit,
    private val onLayoutChanged: (KeypadLayout) -> Unit,
) : LinearLayout(context) {
    private var layout = layouts.firstOrNull { it.key == initial } ?: layouts.first()
    private var symbols = false

    init {
        orientation = VERTICAL
        setBackgroundColor(context.getColor(R.color.app_color_muted))
        val padding = resources.getDimensionPixelSize(R.dimen.app_spacing_xs)
        setPadding(padding, padding, padding, padding)
        render()
    }

    fun release() = removeAllViews()

    private fun render() {
        removeAllViews()
        for (keys in if (symbols) KeypadLayout.SYMBOLS else layout.rows) {
            addView(row(keys.map { character -> textKey(character.toString()) }))
        }
        val bottom = mutableListOf<View>()
        bottom += labelKey(if (symbols) layout.label else "?123") { symbols = !symbols; render() }
        if (layouts.size > 1 && !symbols) {
            bottom += labelKey(nextLayout().label, context.getString(R.string.ime_keypad_switch_layout)) {
                layout = nextLayout()
                onLayoutChanged(layout)
                render()
            }
        }
        bottom += labelKey(" ", context.getString(R.string.ime_keypad_space), weight = 4f) { onText(" ") }
        bottom += backspaceKey()
        bottom += labelKey("✓", context.getString(R.string.ime_keypad_done), weight = 1.5f) { onDone() }
        addView(row(bottom))
    }

    private fun nextLayout(): KeypadLayout = layouts[(layouts.indexOf(layout) + 1) % layouts.size]

    private fun row(keys: List<View>): LinearLayout = LinearLayout(context).apply {
        orientation = HORIZONTAL
        gravity = Gravity.CENTER
        keys.forEach(::addView)
        layoutParams = LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT)
    }

    private fun textKey(character: String): TextView = labelKey(character) { onText(character) }

    private fun labelKey(
        text: String,
        description: String = text,
        weight: Float = 1f,
        onTap: () -> Unit,
    ): TextView = TextView(context).apply {
        this.text = text
        contentDescription = description
        gravity = Gravity.CENTER
        setTextColor(context.getColor(R.color.app_color_foreground))
        setTextSize(android.util.TypedValue.COMPLEX_UNIT_PX, resources.getDimension(R.dimen.app_text_size_large))
        background = PanelStyle.control(context, filled = true)
        layoutParams = keyParams(weight)
        setOnClickListener { onTap() }
    }

    @SuppressLint("ClickableViewAccessibility")
    private fun backspaceKey(): ImageView = ImageView(context).apply {
        setImageResource(R.drawable.ic_lucide_delete)
        contentDescription = context.getString(R.string.ime_keypad_backspace)
        scaleType = ImageView.ScaleType.CENTER
        background = PanelStyle.control(context, filled = true)
        layoutParams = keyParams(1.5f)
        isClickable = true
        setOnClickListener { onBackspace() }
        repeatWhileHeld(this, onBackspace)
    }

    private fun keyParams(weight: Float): LayoutParams {
        val margin = resources.getDimensionPixelSize(R.dimen.app_spacing_xxs)
        return LayoutParams(0, resources.getDimensionPixelSize(R.dimen.app_control_size_keypad_key), weight)
            .apply { setMargins(margin, margin, margin, margin) }
    }
}

/** Repeats [action] while [view] stays pressed, after the platform long-press delay. */
@SuppressLint("ClickableViewAccessibility")
internal fun repeatWhileHeld(view: View, action: () -> Unit) {
    val handler = Handler(Looper.getMainLooper())
    val repeat = object : Runnable {
        override fun run() {
            action()
            handler.postDelayed(this, REPEAT_INTERVAL_MS)
        }
    }
    view.setOnTouchListener { _, event ->
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN ->
                handler.postDelayed(repeat, android.view.ViewConfiguration.getLongPressTimeout().toLong())
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> handler.removeCallbacks(repeat)
        }
        false
    }
    view.addOnAttachStateChangeListener(object : View.OnAttachStateChangeListener {
        override fun onViewAttachedToWindow(v: View) = Unit
        override fun onViewDetachedFromWindow(v: View) = handler.removeCallbacks(repeat)
    })
}

private const val REPEAT_INTERVAL_MS = 60L
