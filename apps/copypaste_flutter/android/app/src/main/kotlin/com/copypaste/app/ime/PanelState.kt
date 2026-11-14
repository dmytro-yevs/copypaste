package com.copypaste.app.ime

import android.content.Context
import android.view.View
import android.view.ViewGroup
import com.copypaste.app.ClipVisibility
import org.json.JSONArray
import org.json.JSONObject

internal enum class PanelTab(val key: String) {
    HISTORY("history"),
    PINNED("pinned"),
    SNIPPETS("snippets"),
    QUEUE("queue"),
    TRANSFORMS("transforms");

    companion object {
        fun of(key: String?): PanelTab = entries.firstOrNull { it.key == key } ?: HISTORY
    }
}

/** One history row as the panel shows it, read from an IPC `Item`. */
internal data class PanelItem(
    val id: String,
    val contentType: String,
    val contentClass: String,
    val semanticKind: String?,
    val content: String,
    val truncated: Boolean,
    val pinned: Boolean,
    val createdAt: Long,
    val sourceAppName: String?,
    val visibility: ClipVisibility,
) {
    val isText: Boolean get() = contentClass == "text"
    val isImage: Boolean get() = contentClass == "image"
    val isHtml: Boolean get() = contentType.startsWith("text/html")

    companion object {
        fun parse(item: JSONObject): PanelItem? {
            val id = item.optString("id").ifEmpty { return null }
            return PanelItem(
                id = id,
                contentType = item.optString("content_type", "text/plain"),
                contentClass = item.optString("content_class", "text"),
                semanticKind = item.optNullableString("semantic_kind"),
                content = item.optString("content"),
                truncated = item.optBoolean("truncated", false),
                pinned = item.optBoolean("pinned", false),
                createdAt = item.optLong("created_at", 0L),
                sourceAppName = item.optNullableString("source_app_name"),
                visibility = ClipVisibility.of(item),
            )
        }
    }
}

internal data class PanelPage(val items: List<PanelItem>, val nextCursor: String?) {
    companion object {
        /** Reads an IPC `ItemPage`; unreadable rows are dropped, never shown. */
        fun parse(page: JSONObject?): PanelPage {
            val rows = page?.optJSONArray("items") ?: JSONArray()
            val items = (0 until rows.length()).mapNotNull { index ->
                rows.optJSONObject(index)?.let(PanelItem::parse)
            }
            return PanelPage(items, page?.optNullableString("next_cursor"))
        }
    }
}

/** What the panel body shows. Every variant except [Ready] renders through one state view. */
internal sealed interface PanelListState {
    data object Loading : PanelListState
    data object Locked : PanelListState
    data class Ready(val items: List<PanelItem>, val hasMore: Boolean) : PanelListState
    data class Empty(val query: String, val pinnedOnly: Boolean) : PanelListState
    data object Failed : PanelListState
}

/** One item action, offered in the long-press sheet and as an accessibility action. */
internal data class PanelAction(val label: String, val run: () -> Unit)

/** Pending paste-queue entries, for the footer indicator and auto-return. */
internal fun pendingQueueEntries(state: JSONObject?): Int {
    val entries = state?.optJSONArray("entries") ?: return 0
    return (0 until entries.length()).count { entries.optJSONObject(it)?.optString("status") == "pending" }
}

/**
 * Device-local panel settings. The Flutter quick-access settings read and
 * write the same keys through its Android channel.
 */
internal class PanelPreferences(context: Context) {
    private val preferences = context.getSharedPreferences(NAME, Context.MODE_PRIVATE)

    var lastTab: PanelTab
        get() = PanelTab.of(preferences.getString(LAST_TAB, null))
        set(value) { preferences.edit().putString(LAST_TAB, value.key).apply() }

    val autoReturn: Boolean get() = preferences.getBoolean(AUTO_RETURN, true)

    /** 0 means the default height for the current screen. */
    var panelHeightDp: Int
        get() = preferences.getInt(PANEL_HEIGHT_DP, 0)
        set(value) { preferences.edit().putInt(PANEL_HEIGHT_DP, value).apply() }

    var keypadLayout: String?
        get() = preferences.getString(KEYPAD_LAYOUT, null)
        set(value) { preferences.edit().putString(KEYPAD_LAYOUT, value).apply() }

    companion object {
        const val NAME = "quick-access-panel"
        const val LAST_TAB = "last-tab"
        const val AUTO_RETURN = "auto-return-after-insert"
        const val PANEL_HEIGHT_DP = "panel-height-dp"
        const val KEYPAD_LAYOUT = "keypad-layout"
    }
}

/**
 * A panel tab whose content another package provides (snippets, queue).
 * The panel owns the header, search, footer and lifecycle; the tab owns its body.
 */
internal interface PanelTabContent {
    val tab: PanelTab
    fun createView(parent: ViewGroup): View
    fun onShown(query: String)
    fun onQueryChanged(query: String)
    /** Drops views, lists and caches; called when the panel hides. */
    fun release()
}

private fun JSONObject.optNullableString(key: String): String? =
    if (has(key) && !isNull(key)) optString(key).ifEmpty { null } else null
