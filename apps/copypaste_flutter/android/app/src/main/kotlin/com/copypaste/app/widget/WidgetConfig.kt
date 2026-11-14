package com.copypaste.app.widget

import android.content.Context

/** What one widget instance lists. */
internal enum class WidgetFilter {
    History,
    Pinned,
    Snippets;

    fun next(): WidgetFilter = entries[(ordinal + 1) % entries.size]
}

/** How much of each row one widget instance shows (quick-access W4). */
internal enum class WidgetPrivacy {
    /** Previews; secrets and concealed snippets stay masked. */
    Full,

    /** Snippet titles; clips show only type, source app and age. */
    Titles,

    /** Only a count and the Save action. */
    Hidden;

    fun next(): WidgetPrivacy = entries[(ordinal + 1) % entries.size]
}

internal data class WidgetConfig(
    val filter: WidgetFilter = WidgetFilter.History,
    val privacy: WidgetPrivacy = WidgetPrivacy.Full,
)

/** Per-instance widget settings, device-local. Holds no clip content. */
internal class WidgetConfigStore(context: Context) {
    private val preferences = context.applicationContext
        .getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)

    fun get(id: Int): WidgetConfig = WidgetConfig(
        filter = preferences.getString(filterKey(id), null)
            ?.let { name -> WidgetFilter.entries.firstOrNull { it.name == name } }
            ?: WidgetFilter.History,
        privacy = preferences.getString(privacyKey(id), null)
            ?.let { name -> WidgetPrivacy.entries.firstOrNull { it.name == name } }
            ?: WidgetPrivacy.Full,
    )

    fun put(id: Int, config: WidgetConfig) {
        preferences.edit()
            .putString(filterKey(id), config.filter.name)
            .putString(privacyKey(id), config.privacy.name)
            .apply()
    }

    fun remove(ids: IntArray) {
        val editor = preferences.edit()
        ids.forEach { editor.remove(filterKey(it)).remove(privacyKey(it)) }
        editor.apply()
    }

    private fun filterKey(id: Int) = "filter-$id"
    private fun privacyKey(id: Int) = "privacy-$id"

    private companion object {
        const val PREFERENCES = "recent-clips-widget"
    }
}
