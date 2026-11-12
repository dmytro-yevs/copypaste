package com.copypaste.app.ime

import android.content.Context
import android.text.format.DateFormat
import android.text.format.DateUtils
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import android.widget.BaseAdapter
import android.widget.LinearLayout
import android.widget.ListView
import android.widget.TextView
import com.copypaste.app.R
import com.copypaste.app.RuntimeAccessWorker
import com.copypaste.app.RuntimeReply
import org.json.JSONArray
import org.json.JSONObject

/** One paste-queue entry as the panel shows it, from an IPC `QueueEntry`. */
internal data class PanelQueueEntry(
    val id: String,
    val status: String,
    val kind: String,
    val preview: String,
    val masked: Boolean,
    val edited: Boolean,
    val sourceAppName: String?,
    val renderedAt: Long?,
) {
    val pending: Boolean get() = status == "pending"
    val textual: Boolean get() = kind == "text" || kind == "rich_text" || kind == "link"

    companion object {
        fun parse(entry: JSONObject): PanelQueueEntry? = PanelQueueEntry(
            id = entry.optString("entry_id").ifEmpty { return null },
            status = entry.optString("status"),
            kind = entry.optString("content_kind", "text"),
            preview = entry.optString("preview"),
            masked = entry.optBoolean("masked", false),
            edited = entry.optBoolean("edited", false),
            sourceAppName = entry.nullableString("source_app_name"),
            renderedAt = entry.takeIf { it.has("rendered_at") && !it.isNull("rendered_at") }?.optLong("rendered_at"),
        )
    }
}

/** The IPC `PasteQueueState` the tab renders; [settings] is kept whole for settings patches. */
internal data class PanelQueueState(
    val collecting: Boolean,
    val full: Boolean,
    val entries: List<PanelQueueEntry>,
    val staleSince: Long?,
    val settings: JSONObject,
) {
    val enabled: Boolean get() = settings.optBoolean("enabled", true)
    val pending: List<PanelQueueEntry> get() = entries.filter(PanelQueueEntry::pending)
    val head: PanelQueueEntry? get() = entries.firstOrNull(PanelQueueEntry::pending)

    companion object {
        fun parse(state: JSONObject?): PanelQueueState? {
            state ?: return null
            return PanelQueueState(
                collecting = state.optBoolean("collecting", false),
                full = state.optBoolean("full", false),
                entries = state.optJSONArray("entries").objects().mapNotNull(PanelQueueEntry::parse),
                staleSince = state.takeIf { it.has("stale_since") && !it.isNull("stale_since") }?.optLong("stale_since"),
                settings = state.optJSONObject("settings") ?: JSONObject(),
            )
        }
    }
}

/**
 * The Queue tab: Insert next commits the head entry and marks it pasted
 * only once the editor accepted the commit; Paste all joins text entries.
 * The runtime owns the queue; the tab keeps nothing once the panel hides.
 */
internal class PanelQueueTab(
    private val context: Context,
    private val host: PanelTabHost,
    private val worker: RuntimeAccessWorker,
    private val clock: () -> Long = System::currentTimeMillis,
) : PanelTabContent {
    override val tab = PanelTab.QUEUE
    private var state: PanelQueueState? = null
    private var views: Views? = null
    private val adapter = Adapter()
    private var generation = 0

    private class Views(root: View) {
        val options: LinearLayout = root.findViewById(R.id.ime_queue_options)
        val status: TextView = root.findViewById(R.id.ime_queue_status)
        val list: ListView = root.findViewById(R.id.ime_queue_list)
        val actions: LinearLayout = root.findViewById(R.id.ime_queue_actions)
        val state = FeatureState(root)
    }

    override fun createView(parent: ViewGroup): View {
        val root = LayoutInflater.from(context).inflate(R.layout.ime_queue_tab, parent, false)
        val created = Views(root)
        created.list.adapter = adapter
        created.list.setOnItemClickListener { _, _, position, _ -> entries().getOrNull(position)?.let(::insertEntry) }
        created.list.setOnItemLongClickListener { _, _, position, _ ->
            entries().getOrNull(position)?.let { host.showActions(summary(it), actionsFor(it)) }
            true
        }
        views = created
        return root
    }

    override fun onShown(query: String) {
        render()
        refresh()
    }

    override fun onQueryChanged(query: String) = Unit

    override fun release() {
        generation += 1
        state = null
        views = null
        adapter.notifyDataSetChanged()
    }

    /** "Add to queue" on a history item; a secret is queued only after unlocking (K22). */
    fun itemAction(item: PanelItem): PanelAction = PanelAction(context.getString(R.string.ime_action_add_to_queue)) {
        val enqueueClip = { enqueue(JSONObject().put("kind", "clip").put("id", item.id)) }
        if (item.visibility.masked) host.unlockSecrets(enqueueClip) else enqueueClip()
    }

    fun enqueue(source: JSONObject) {
        queueOp(opOf("add").put("sources", JSONArray().put(source))) { applied ->
            host.showNotice(context.getString(if (applied) R.string.ime_queue_added else R.string.ime_queue_add_failed))
        }
    }

    fun insertNext() {
        val state = state ?: return
        val head = state.head ?: return host.showNotice(context.getString(R.string.ime_queue_nothing))
        val reason = insertBlocked(head)
        if (reason != null) {
            if (head.kind == FILES) {
                host.showNotice(context.getString(reason), context.getString(R.string.ime_action_copy)) { copyEntry(head) }
            } else {
                host.showNotice(context.getString(reason))
            }
            return
        }
        val target = JSONObject().put("kind", "next")
        unlocked(head.masked) { if (head.kind == IMAGE) pasteViaClipboard(target) else deliverAndCommit(target) }
    }

    fun pasteAll() {
        val pending = state?.pending.orEmpty()
        if (pending.isEmpty()) return host.showNotice(context.getString(R.string.ime_queue_nothing))
        if (!pending.all(PanelQueueEntry::textual)) return host.showNotice(context.getString(R.string.ime_queue_paste_all_mixed))
        val secrets = pending.count(PanelQueueEntry::masked)
        val all = JSONObject().put("kind", "all")
        if (secrets == 0) return deliverAndCommit(all)
        // Secrets join a multi-paste only with explicit consent and the device owner (K22).
        host.showActions(
            context.resources.getQuantityString(R.plurals.ime_queue_secret_confirm, secrets, secrets),
            listOf(PanelAction(context.getString(R.string.ime_queue_include_secrets)) { host.unlockSecrets { deliverAndCommit(all) } }),
        )
    }

    fun actionsFor(entry: PanelQueueEntry): List<PanelAction> = buildList {
        fun label(id: Int) = context.getString(id)
        val index = entries().indexOf(entry)
        if (entry.pending) add(PanelAction(label(R.string.ime_queue_insert_entry)) { insertEntry(entry) })
        if (entry.status != "unavailable") add(PanelAction(label(R.string.ime_action_copy)) { copyEntry(entry) })
        if (index > 0) add(PanelAction(label(R.string.ime_queue_move_up)) { move(entry, index - 1) })
        if (index >= 0 && index < entries().size - 1) add(PanelAction(label(R.string.ime_queue_move_down)) { move(entry, index + 1) })
        add(PanelAction(label(R.string.ime_queue_remove)) {
            queueOp(opOf("remove").put("entry_ids", JSONArray().put(entry.id)))
        })
    }

    private fun entries(): List<PanelQueueEntry> = state?.entries.orEmpty()

    private fun refresh() {
        val requested = ++generation
        worker.op("paste_queue", opOf("get")) { reply ->
            if (requested != generation) return@op
            apply(reply)
        }
    }

    /** Runs a queue op that answers with the new state. */
    private fun queueOp(op: JSONObject, then: ((Boolean) -> Unit)? = null) {
        worker.op("paste_queue", op) { reply ->
            val applied = reply is RuntimeReply.Ok
            if (applied) apply(reply) else if (then == null) host.showNotice(context.getString(R.string.ime_queue_failed))
            host.queueChanged()
            then?.invoke(applied)
        }
    }

    private fun apply(reply: RuntimeReply) {
        val parsed = (reply as? RuntimeReply.Ok)?.takeIf { it.kind == "paste_queue" }?.let { PanelQueueState.parse(it.objectData()) }
        if (reply is RuntimeReply.Ok && parsed == null) return
        if (views == null) return
        state = parsed
        render(failed = parsed == null)
    }

    private fun insertBlocked(entry: PanelQueueEntry): Int? = when {
        entry.kind == FILES -> R.string.ime_queue_files_not_insertable
        entry.kind == IMAGE && !host.field.accepts(PanelController.IMAGE_PNG) -> R.string.ime_image_not_accepted
        else -> null
    }

    private fun insertEntry(entry: PanelQueueEntry) {
        if (!entry.pending) return
        val reason = insertBlocked(entry)
        if (reason != null) return host.showNotice(context.getString(reason))
        val target = JSONObject().put("kind", "entry").put("id", entry.id)
        unlocked(entry.masked) { if (entry.kind == IMAGE) pasteViaClipboard(target) else deliverAndCommit(target) }
    }

    private fun unlocked(secret: Boolean, action: () -> Unit) = if (secret) host.unlockSecrets(action) else action()

    /**
     * Takes the text without touching the clipboard, commits it, and only
     * then confirms; a refused commit leaves the entries pending (R44).
     */
    private fun deliverAndCommit(target: JSONObject) {
        val total = entries().size
        worker.op("paste_queue", deliverOp(target, "return")) { reply ->
            val delivery = (reply as? RuntimeReply.Ok)?.objectData() ?: return@op deliveryFailed()
            noticeUnavailable(delivery)
            val nothing = delivery.nullableString("nothing_to_paste")
            val token = delivery.nullableString("token")
            val text = delivery.nullableString("text")
            if (nothing != null || token == null || text == null) {
                host.queueChanged()
                refresh()
                return@op nothingToPaste(nothing)
            }
            val outcome = host.insert.insertText(text, host.field)
            val inserted = outcome == InsertOutcome.INSERTED
            confirm(token, inserted) {
                if (!inserted) return@confirm
                val pasted = state?.entries?.count { it.status == "pasted" } ?: 0
                host.announce(context.getString(R.string.ime_queue_pasted, pasted, total))
                if (state?.settings?.optBoolean("move_to_next_field", false) == true) {
                    host.inputConnection()?.performEditorAction(EditorInfo.IME_ACTION_NEXT)
                }
            }
            val retry = delivery.optJSONArray("entry_ids").strings()
            host.afterInsert(outcome) {
                pasteViaClipboard(if (retry.size == 1) JSONObject().put("kind", "entry").put("id", retry.single()) else target)
            }
        }
    }

    /** Images, and the explicit fallback: a transient clipboard write, then the editor's paste. */
    private fun pasteViaClipboard(target: JSONObject) {
        worker.op("paste_queue", deliverOp(target, "clipboard")) { reply ->
            val delivery = (reply as? RuntimeReply.Ok)?.objectData() ?: return@op deliveryFailed()
            noticeUnavailable(delivery)
            val token = delivery.nullableString("token") ?: return@op nothingToPaste(delivery.nullableString("nothing_to_paste"))
            confirm(token, host.insert.pasteFromClipboard()) {}
        }
    }

    private fun copyEntry(entry: PanelQueueEntry) = unlocked(entry.masked) {
        worker.op("paste_queue", deliverOp(JSONObject().put("kind", "entry").put("id", entry.id), "copy_only")) { reply ->
            if (reply is RuntimeReply.Ok) host.announce(context.getString(R.string.ime_copied)) else deliveryFailed()
            refresh()
            host.queueChanged()
        }
    }

    private fun confirm(token: String, inserted: Boolean, then: () -> Unit) {
        val op = opOf("confirm").put("token", token).put("outcome", if (inserted) "inserted" else "rejected")
        queueOp(op) { then() }
    }

    private fun deliverOp(target: JSONObject, mode: String): JSONObject =
        opOf("deliver").put("target", target).put("plain_text", JSONObject.NULL).put("mode", mode)

    private fun noticeUnavailable(delivery: JSONObject) {
        val gone = delivery.optJSONObject("notices")?.optInt("unavailable", 0) ?: 0
        if (gone > 0) host.showNotice(context.resources.getQuantityString(R.plurals.ime_queue_unavailable, gone, gone))
    }

    private fun nothingToPaste(reason: String?) = host.showNotice(
        context.getString(
            when (reason) {
                "finished" -> R.string.ime_queue_finished
                "mixed" -> R.string.ime_queue_paste_all_mixed
                else -> R.string.ime_queue_nothing
            },
        ),
    )

    private fun deliveryFailed() {
        host.showNotice(context.getString(R.string.ime_queue_failed))
        refresh()
    }

    private fun move(entry: PanelQueueEntry, index: Int) =
        queueOp(opOf("move").put("entry_id", entry.id).put("to_index", index))

    private fun setCollecting(on: Boolean) = queueOp(opOf("set_collecting").put("on", on))

    private fun clear() = queueOp(opOf("clear")) { cleared ->
        if (cleared) {
            host.showNotice(context.getString(R.string.ime_queue_cleared), context.getString(R.string.ime_queue_undo)) {
                queueOp(opOf("undo_clear"))
            }
        }
    }

    /** Queue settings are device settings; a patch sends the whole queue block back. */
    private fun updateSettings(change: (JSONObject) -> Unit) {
        val settings = JSONObject(state?.settings?.toString() ?: return).also(change)
        worker.request("set_config", JSONObject().put("patch", JSONObject().put("paste_queue", settings))) { reply ->
            if (reply !is RuntimeReply.Ok) host.showNotice(context.getString(R.string.ime_queue_failed))
            refresh()
        }
    }

    private fun render(failed: Boolean = false) {
        val views = views ?: return
        adapter.notifyDataSetChanged()
        views.options.removeAllViews()
        views.actions.removeAllViews()
        val state = state
        when {
            failed -> views.state.show(R.drawable.ic_lucide_list_ordered, context.getString(R.string.ime_queue_error), context.getString(R.string.ime_retry)) { refresh() }
            state == null -> views.state.show(R.drawable.ic_lucide_list_ordered, context.getString(R.string.ime_queue_loading))
            !state.enabled -> views.state.show(R.drawable.ic_lucide_list_ordered, context.getString(R.string.ime_queue_off))
            state.entries.isEmpty() -> views.state.show(R.drawable.ic_lucide_list_ordered, context.getString(R.string.ime_queue_empty))
            else -> views.state.hide()
        }
        views.status.text = state?.let(::status).orEmpty()
        if (state == null || !state.enabled) return
        renderOptions(views.options, state)
        renderActions(views.actions, state)
    }

    private fun renderOptions(options: LinearLayout, state: PanelQueueState) {
        options.addView(FeatureControls.chip(context, context.getString(R.string.ime_queue_collect), selected = state.collecting) {
            setCollecting(!state.collecting)
        })
        val separator = state.settings.optString("separator", "new_line")
        val separatorName = context.getString(separatorLabel(separator))
        options.addView(FeatureControls.chip(context, context.getString(R.string.ime_queue_separator, separatorName)) {
            updateSettings { it.put("separator", nextSeparator(separator)) }
        })
        val nextField = state.settings.optBoolean("move_to_next_field", false)
        options.addView(FeatureControls.chip(context, context.getString(R.string.ime_queue_next_field), selected = nextField) {
            updateSettings { it.put("move_to_next_field", !nextField) }
        })
        if (state.entries.isEmpty()) return
        options.addView(FeatureControls.chip(context, context.getString(R.string.ime_queue_reverse)) { queueOp(opOf("reverse")) })
        options.addView(FeatureControls.chip(context, context.getString(R.string.ime_queue_clear)) { clear() })
    }

    private fun renderActions(actions: LinearLayout, state: PanelQueueState) {
        val head = state.head
        val reason = head?.let(::insertBlocked)
        val insertNext = FeatureControls.action(context, context.getString(R.string.ime_queue_insert_next), weight = 2f) { insertNext() }
        insertNext.background = PanelStyle.control(context, filled = true, selected = true)
        FeatureControls.setAvailable(insertNext, head != null && reason == null)
        insertNext.contentDescription = listOfNotNull(
            context.getString(R.string.ime_queue_insert_next),
            reason?.let(context::getString),
        ).joinToString(", ")
        actions.addView(insertNext)
        val pending = state.pending
        val pasteAll = FeatureControls.action(context, context.getString(R.string.ime_queue_paste_all)) { pasteAll() }
        FeatureControls.setAvailable(pasteAll, pending.size > 1 && pending.all(PanelQueueEntry::textual))
        actions.addView(pasteAll)
    }

    /** Position, or the reason Insert next cannot act, plus finished, full, stale and collecting hints. */
    private fun status(state: PanelQueueState): String {
        val head = state.head
        val parts = mutableListOf<String>()
        when {
            head == null && state.entries.isNotEmpty() -> parts += context.getString(R.string.ime_queue_finished)
            head != null -> {
                val reason = insertBlocked(head)
                parts += if (reason != null) {
                    context.getString(reason)
                } else {
                    context.getString(R.string.ime_queue_position, state.entries.indexOf(head) + 1, state.entries.size)
                }
            }
        }
        if (state.full) parts += context.getString(R.string.ime_queue_full)
        state.staleSince?.let {
            val age = DateUtils.getRelativeTimeSpanString(it, clock(), DateUtils.MINUTE_IN_MILLIS).toString()
            parts += context.getString(R.string.ime_queue_stale, age)
        }
        if (state.collecting) parts += context.getString(R.string.ime_queue_collect_hint)
        return parts.joinToString(" · ")
    }

    private fun summary(entry: PanelQueueEntry): String = when {
        entry.masked -> context.getString(R.string.ime_masked_summary, kindLabel(entry))
        entry.preview.isBlank() -> kindLabel(entry)
        else -> entry.preview.trim().take(PREVIEW_CHARS)
    }

    private fun kindLabel(entry: PanelQueueEntry): String = context.getString(
        when (entry.kind) {
            "link" -> R.string.ime_kind_link
            IMAGE -> R.string.ime_kind_image
            FILES -> R.string.ime_kind_file
            else -> R.string.ime_kind_text
        },
    )

    private fun meta(entry: PanelQueueEntry, position: Int): String = listOfNotNull(
        position.toString(),
        when (entry.status) {
            "pasted" -> context.getString(R.string.ime_queue_status_pasted)
            "skipped" -> context.getString(R.string.ime_queue_status_skipped)
            "unavailable" -> context.getString(R.string.ime_queue_status_unavailable)
            else -> null
        },
        entry.sourceAppName,
        entry.renderedAt?.let { context.getString(R.string.ime_queue_rendered_at, DateFormat.getTimeFormat(context).format(java.util.Date(it))) },
        context.getString(R.string.ime_queue_edited).takeIf { entry.edited },
    ).joinToString(" · ")

    private inner class Adapter : BaseAdapter() {
        override fun getCount(): Int = if (views == null) 0 else entries().size
        override fun getItem(position: Int): PanelQueueEntry = entries()[position]
        override fun getItemId(position: Int): Long = entries()[position].id.hashCode().toLong()
        override fun hasStableIds(): Boolean = true

        override fun getView(position: Int, convertView: View?, parent: ViewGroup): View {
            val row = convertView ?: LayoutInflater.from(context).inflate(R.layout.ime_clip_row, parent, false)
            val entry = entries()[position]
            val meta = meta(entry, position + 1)
            val text = summary(entry)
            val description = listOfNotNull(
                if (entry.masked) context.getString(R.string.ime_secret_item) else kindLabel(entry),
                text.takeIf { !entry.masked },
                meta,
            ).joinToString(", ")
            bindFeatureRow(row, text, meta, if (entry.masked) R.drawable.ic_lucide_lock else 0, description, usable = entry.pending)
            bindRowActions(row, actionsFor(entry))
            return row
        }
    }

    companion object {
        private const val IMAGE = "image"
        private const val FILES = "files"
        private const val PREVIEW_CHARS = 200
        private val SEPARATORS = listOf("new_line", "space", "tab", "comma", "none")

        fun nextSeparator(current: String): String =
            SEPARATORS.getOrElse((SEPARATORS.indexOf(current) + 1) % SEPARATORS.size) { SEPARATORS.first() }

        fun separatorLabel(separator: String): Int = when (separator) {
            "blank_line" -> R.string.ime_separator_blank_line
            "tab" -> R.string.ime_separator_tab
            "space" -> R.string.ime_separator_space
            "comma" -> R.string.ime_separator_comma
            "comma_space" -> R.string.ime_separator_comma_space
            "none" -> R.string.ime_separator_none
            "custom" -> R.string.ime_separator_custom
            else -> R.string.ime_separator_new_line
        }
    }
}
