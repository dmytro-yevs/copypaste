package com.copypaste.app.widget

import android.content.Context
import com.copypaste.app.QuickPasteSurface

/** What one widget instance lists: History, Pinned, or a module's Quick Paste surface. */
internal sealed interface WidgetFilter {
    /** The stored form. */
    val key: String

    data object History : WidgetFilter {
        override val key = "History"
    }

    data object Pinned : WidgetFilter {
        override val key = "Pinned"
    }

    /** [title] is the surface's label when the filter was chosen. */
    data class Module(val id: String, val title: String) : WidgetFilter {
        override val key: String get() = MODULE_PREFIX + id
    }

    /** The filter after this one among History, Pinned and the modules' [surfaces]. */
    fun next(surfaces: List<QuickPasteSurface>): WidgetFilter {
        val all = listOf(History, Pinned) + surfaces.map { Module(it.moduleId, it.title) }
        return all[(all.indexOfFirst { it.key == key } + 1) % all.size]
    }

    companion object {
        private const val MODULE_PREFIX = "module:"

        /** A stored filter; an unknown one, such as a retired built-in filter, lists History. */
        fun of(key: String?, title: String?): WidgetFilter = when {
            key == Pinned.key -> Pinned
            key != null && key.startsWith(MODULE_PREFIX) && key.length > MODULE_PREFIX.length -> {
                val id = key.removePrefix(MODULE_PREFIX)
                Module(id, title ?: id)
            }
            else -> History
        }
    }
}

/** How much of each row one widget instance shows (quick-access W4). */
internal enum class WidgetPrivacy {
    /** Previews; secrets and secret module results stay masked. */
    Full,

    /** Module result titles; clips show only type, source app and age. */
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
        filter = WidgetFilter.of(preferences.getString(filterKey(id), null), preferences.getString(filterTitleKey(id), null)),
        privacy = preferences.getString(privacyKey(id), null)
            ?.let { name -> WidgetPrivacy.entries.firstOrNull { it.name == name } }
            ?: WidgetPrivacy.Full,
    )

    fun put(id: Int, config: WidgetConfig) {
        val editor = preferences.edit()
            .putString(filterKey(id), config.filter.key)
            .putString(privacyKey(id), config.privacy.name)
        when (val filter = config.filter) {
            is WidgetFilter.Module -> editor.putString(filterTitleKey(id), filter.title)
            else -> editor.remove(filterTitleKey(id))
        }
        editor.apply()
    }

    fun remove(ids: IntArray) {
        val editor = preferences.edit()
        ids.forEach { editor.remove(filterKey(it)).remove(filterTitleKey(it)).remove(privacyKey(it)) }
        editor.apply()
    }

    private fun filterKey(id: Int) = "filter-$id"
    private fun filterTitleKey(id: Int) = "filter-title-$id"
    private fun privacyKey(id: Int) = "privacy-$id"

    private companion object {
        const val PREFERENCES = "recent-clips-widget"
    }
}
