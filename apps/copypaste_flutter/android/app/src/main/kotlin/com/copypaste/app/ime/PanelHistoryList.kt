package com.copypaste.app.ime

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.text.Html
import android.text.format.DateUtils
import android.util.Base64
import android.util.LruCache
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.BaseAdapter
import android.widget.ImageView
import android.widget.TextView
import androidx.core.view.ViewCompat
import com.copypaste.app.AndroidCaptureFeedback
import com.copypaste.app.ClipPresentation
import com.copypaste.app.R
import com.copypaste.app.RuntimeAccessWorker
import com.copypaste.app.RuntimeReply
import org.json.JSONObject

/**
 * The History and Pinned tab body: cursor-paged history (30 per page),
 * searched by the runtime, with row binding and bounded thumbnails.
 */
internal class PanelHistoryList(
    private val context: Context,
    private val worker: RuntimeAccessWorker,
    private val onState: (PanelListState) -> Unit,
    private val clock: () -> Long = System::currentTimeMillis,
) {
    private val items = mutableListOf<PanelItem>()
    private var generation = 0
    private var cursor: String? = null
    private var loadingMore = false
    private var query = ""
    private var pinnedOnly = false
    private val thumbnails = object : LruCache<String, Bitmap>(THUMBNAIL_CACHE_BYTES) {
        override fun sizeOf(key: String, value: Bitmap): Int = value.byteCount
    }
    private val pendingThumbnails = mutableSetOf<String>()

    /** Masks every row, for password fields. */
    var maskedMode = false
    var secretsRevealed = false
    /** Text of the system clip, to mark the row that matches it. */
    var currentClipText: String? = null
    var imagesAccepted = false
    var actionsFor: (PanelItem) -> List<PanelAction> = { emptyList() }

    val adapter = Adapter()
    val loaded: List<PanelItem> get() = items

    fun load(query: String, pinnedOnly: Boolean) {
        generation += 1
        this.query = query
        this.pinnedOnly = pinnedOnly
        cursor = null
        loadingMore = false
        // Earlier results stay visible until the new page replaces them.
        if (items.isEmpty()) onState(PanelListState.Loading)
        request(generation, after = null)
    }

    fun reload() = load(query, pinnedOnly)

    fun loadMore() {
        val after = cursor ?: return
        if (loadingMore) return
        loadingMore = true
        request(generation, after)
    }

    fun remove(id: String) {
        if (items.removeAll { it.id == id }) publish()
    }

    fun release() {
        generation += 1
        items.clear()
        cursor = null
        currentClipText = null
        thumbnails.evictAll()
        pendingThumbnails.clear()
        adapter.notifyDataSetChanged()
    }

    private fun request(requestGeneration: Int, after: String?) {
        val search = JSONObject().put("pinned_only", pinnedOnly).put("sort", "newest")
        if (query.isNotBlank()) search.put("search", query.trim())
        val params = JSONObject().put("query", search).put("limit", PAGE_SIZE)
        if (after != null) params.put("cursor", after)
        worker.request("history_query", params) { reply ->
            if (requestGeneration != generation) return@request
            loadingMore = false
            when (reply) {
                is RuntimeReply.Ok -> {
                    val page = PanelPage.parse(reply.objectData())
                    if (after == null) items.clear()
                    val now = clock()
                    items += page.items.filter {
                        it.visibility.presentationAt(now) != ClipPresentation.Hidden && items.none { known -> known.id == it.id }
                    }
                    cursor = page.nextCursor
                    publish()
                }
                is RuntimeReply.Failure -> if (after == null) {
                    items.clear()
                    adapter.notifyDataSetChanged()
                    onState(PanelListState.Failed)
                }
            }
        }
    }

    private fun publish() {
        adapter.notifyDataSetChanged()
        onState(
            if (items.isEmpty()) PanelListState.Empty(query.trim(), pinnedOnly)
            else PanelListState.Ready(items.toList(), cursor != null),
        )
    }

    fun presentation(item: PanelItem): ClipPresentation {
        val presentation = item.visibility.presentationAt(clock(), maskedMode)
        val revealed = secretsRevealed && !maskedMode && item.visibility.masked
        return if (presentation == ClipPresentation.Masked && revealed) ClipPresentation.Full else presentation
    }

    fun summary(item: PanelItem): String = when {
        presentation(item) != ClipPresentation.Full -> maskedSummary(context, item)
        item.isImage -> context.getString(R.string.ime_kind_image)
        item.isHtml -> AndroidCaptureFeedback.textPreview(Html.fromHtml(item.content, Html.FROM_HTML_MODE_LEGACY).toString().trim())
        else -> AndroidCaptureFeedback.textPreview(item.content.trim())
    }

    inner class Adapter : BaseAdapter() {
        override fun getCount(): Int = items.size
        override fun getItem(position: Int): PanelItem = items[position]
        override fun getItemId(position: Int): Long = items[position].id.hashCode().toLong()
        override fun hasStableIds(): Boolean = true

        override fun getView(position: Int, convertView: View?, parent: ViewGroup): View {
            val row = convertView ?: LayoutInflater.from(context).inflate(R.layout.ime_clip_row, parent, false)
            bind(row, items[position])
            if (position >= items.size - LOAD_MORE_THRESHOLD) loadMore()
            return row
        }
    }

    private fun bind(row: View, item: PanelItem) {
        val presentation = presentation(item)
        val text = row.findViewById<TextView>(R.id.ime_row_text)
        val meta = row.findViewById<TextView>(R.id.ime_row_meta)
        val thumbnail = row.findViewById<ImageView>(R.id.ime_row_thumbnail)
        val badge = row.findViewById<ImageView>(R.id.ime_row_badge)
        val summary = summary(item)
        text.text = summary
        val current = presentation == ClipPresentation.Full && item.isText && !item.truncated &&
            currentClipText != null && currentClipText == item.content
        meta.text = metadata(item, current)
        val image = if (presentation == ClipPresentation.Full && item.isImage) thumbnail(item) else null
        thumbnail.setImageBitmap(image)
        thumbnail.visibility = if (image != null) View.VISIBLE else View.GONE
        when {
            item.visibility.masked -> badge.setImageResource(R.drawable.ic_lucide_lock)
            item.pinned -> badge.setImageResource(R.drawable.ic_lucide_pin)
            else -> badge.setImageDrawable(null)
        }
        badge.visibility = if (item.visibility.masked || item.pinned) View.VISIBLE else View.GONE
        val usable = item.isText || (item.isImage && imagesAccepted)
        row.alpha = if (usable) 1f else DISABLED_ALPHA
        row.contentDescription = description(item, presentation, summary, meta.text.toString())
        bindAccessibilityActions(row, item)
    }

    private fun metadata(item: PanelItem, current: Boolean): String = listOfNotNull(
        context.getString(R.string.ime_current_clipboard).takeIf { current },
        item.sourceAppName,
        item.createdAt.takeIf { it > 0 }?.let {
            DateUtils.getRelativeTimeSpanString(it, clock(), DateUtils.MINUTE_IN_MILLIS).toString()
        },
    ).joinToString(" · ")

    /** Type, summary, source, age and state; a masked clip announces no content. */
    private fun description(item: PanelItem, presentation: ClipPresentation, summary: String, metadata: String): String {
        val masked = presentation != ClipPresentation.Full
        return listOfNotNull(
            if (masked && item.visibility.masked) context.getString(R.string.ime_secret_item) else kindLabel(context, item),
            summary.takeIf { !masked && !item.isImage },
            maskedSummary(context, item).takeIf { masked && !item.visibility.masked },
            metadata.takeIf { it.isNotEmpty() },
            context.getString(R.string.ime_pinned).takeIf { item.pinned },
        ).joinToString(", ")
    }

    private fun bindAccessibilityActions(row: View, item: PanelItem) {
        (row.getTag(R.id.ime_row_text) as? List<*>)?.forEach { id ->
            if (id is Int) ViewCompat.removeAccessibilityAction(row, id)
        }
        val ids = actionsFor(item).map { action ->
            ViewCompat.addAccessibilityAction(row, action.label) { _, _ -> action.run(); true }
        }
        row.setTag(R.id.ime_row_text, ids)
    }

    private fun thumbnail(item: PanelItem): Bitmap? {
        thumbnails.get(item.id)?.let { return it }
        if (!pendingThumbnails.add(item.id)) return null
        val edge = context.resources.getDimensionPixelSize(R.dimen.app_icon_size_xl)
        val requestGeneration = generation
        val params = JSONObject().put("id", item.id)
            .put("bounds", JSONObject().put("width", edge).put("height", edge))
        worker.request("image_preview", params) { reply ->
            pendingThumbnails.remove(item.id)
            if (requestGeneration != generation) return@request
            val encoded = (reply as? RuntimeReply.Ok)?.objectData()?.optString("png_base64").orEmpty()
            val bytes = runCatching { Base64.decode(encoded, Base64.DEFAULT) }.getOrNull() ?: return@request
            val bitmap = runCatching { BitmapFactory.decodeByteArray(bytes, 0, bytes.size) }.getOrNull()
                ?: return@request
            thumbnails.put(item.id, bitmap)
            adapter.notifyDataSetChanged()
        }
        return null
    }

    companion object {
        const val PAGE_SIZE = 30
        private const val LOAD_MORE_THRESHOLD = 5
        private const val THUMBNAIL_CACHE_BYTES = 2 * 1024 * 1024
        private const val DISABLED_ALPHA = 0.5f
    }
}

internal fun kindLabel(context: Context, item: PanelItem): String = context.getString(
    when {
        item.isImage -> R.string.ime_kind_image
        !item.isText -> R.string.ime_kind_file
        else -> when (item.semanticKind) {
            "link" -> R.string.ime_kind_link
            "email" -> R.string.ime_kind_email
            "phone" -> R.string.ime_kind_phone
            "color" -> R.string.ime_kind_color
            "code", "json" -> R.string.ime_kind_code
            else -> R.string.ime_kind_text
        }
    },
)

/** Dots plus a type and length hint; never any content (K26). */
internal fun maskedSummary(context: Context, item: PanelItem): String {
    val kind = kindLabel(context, item)
    if (!item.isText || item.truncated) return context.getString(R.string.ime_masked_summary, kind)
    val length = item.content.codePointCount(0, item.content.length)
    return context.getString(
        R.string.ime_masked_summary_length,
        kind,
        context.resources.getQuantityString(R.plurals.ime_characters, length, length),
    )
}
