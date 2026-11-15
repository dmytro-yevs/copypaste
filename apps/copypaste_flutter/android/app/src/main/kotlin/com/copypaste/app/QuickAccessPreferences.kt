package com.copypaste.app

import android.content.Context
import com.copypaste.app.ime.PanelPreferences

/** Keyboard panel options in the panel's own store: the app's settings write them, the panel reads them. */
internal object QuickAccessPreferences {
    private const val STORE = PanelPreferences.NAME

    enum class PanelOption(val wire: String, val key: String, val default: Boolean) {
        AUTO_RETURN("autoReturn", PanelPreferences.AUTO_RETURN, true),
        FOCUS_SEARCH_ON_OPEN("focusSearchOnOpen", "focus-search-on-open", false);

        companion object {
            fun fromWire(wire: String?): PanelOption? = entries.firstOrNull { it.wire == wire }
        }
    }

    fun enabled(context: Context, option: PanelOption): Boolean =
        context.getSharedPreferences(STORE, Context.MODE_PRIVATE).getBoolean(option.key, option.default)

    fun setEnabled(context: Context, option: PanelOption, enabled: Boolean): Boolean =
        context.getSharedPreferences(STORE, Context.MODE_PRIVATE)
            .edit().putBoolean(option.key, enabled).commit()

    fun asMap(context: Context): Map<String, Boolean> =
        PanelOption.entries.associate { it.wire to enabled(context, it) }
}
