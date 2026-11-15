package com.copypaste.app.ime

import android.content.Context
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.BaseAdapter
import android.widget.FrameLayout
import android.widget.ListView
import com.copypaste.app.AndroidClipboardWriter
import com.copypaste.app.ModuleAction
import com.copypaste.app.ModuleDelivery
import com.copypaste.app.ModuleForm
import com.copypaste.app.ModuleOutput
import com.copypaste.app.ModuleResult
import com.copypaste.app.ModuleSurfaces
import com.copypaste.app.QuickPasteSurface
import com.copypaste.app.R
import com.copypaste.app.RuntimeAccessWorker
import com.copypaste.app.deliverTextOp
import com.copypaste.app.invokeModule
import java.text.BreakIterator
import org.json.JSONObject

/**
 * The tabs of installed modules' Quick Paste surfaces (module contract v7),
 * read on every panel show. A tab whose surface did not change keeps its
 * instance, so the shown tab is not rebuilt.
 */
internal class PanelModuleTabs(
    private val context: Context,
    private val host: PanelTabHost,
    private val worker: RuntimeAccessWorker,
    private val transforms: PanelTransforms,
    private val queue: PanelQueueTab,
) : PanelTabSource {
    private var tabs: List<PanelModuleTab> = emptyList()

    override fun load(onTabs: (List<PanelTabContent>) -> Unit) {
        worker.perform({ access -> ModuleSurfaces.quickPaste(access) }) { surfaces ->
            // An unreadable list keeps the tabs already shown.
            if (surfaces != null) {
                tabs = surfaces.map { surface ->
                    tabs.firstOrNull { it.surface == surface } ?: PanelModuleTab(context, host, worker, transforms, queue, surface)
                }
            }
            onTabs(tabs)
        }
    }
}

/**
 * One module's Quick Paste tab: the module's search results, its form inside
 * the panel, and insertion by commit. The module ranks and renders; the
 * panel inserts, copies or queues what it delivers.
 */
internal class PanelModuleTab(
    private val context: Context,
    private val host: PanelTabHost,
    private val worker: RuntimeAccessWorker,
    private val transforms: PanelTransforms,
    private val queue: PanelQueueTab,
    val surface: QuickPasteSurface,
) : PanelTabContent {
    override val tab = PanelTab.module(surface.moduleId)
    override val title: String get() = surface.title
    private val rows = mutableListOf<ModuleResult>()
    private val adapter = Adapter()
    private var list: ListView? = null
    private var state: FeatureState? = null
    private var formSlot: FrameLayout? = null
    private var form: PanelModuleForm? = null
    private var query = ""
    private var generation = 0

    override fun createView(parent: ViewGroup): View {
        val view = LayoutInflater.from(context).inflate(R.layout.ime_module_tab, parent, false)
        val list = view.findViewById<ListView>(R.id.ime_module_list)
        list.adapter = adapter
        list.setOnItemClickListener { _, _, position, _ -> rows.getOrNull(position)?.let(::insert) }
        list.setOnItemLongClickListener { _, _, position, _ ->
            rows.getOrNull(position)?.let { host.showActions(it.title, actionsFor(it)) }
            true
        }
        this.list = list
        state = FeatureState(view)
        formSlot = view.findViewById(R.id.ime_module_form_slot)
        form?.release()
        form = null
        return view
    }

    override fun onShown(query: String) = load(query)

    override fun onQueryChanged(query: String) = load(query)

    override fun release() {
        generation += 1
        form?.release()
        form = null
        rows.clear()
        list = null
        state = null
        formSlot = null
        adapter.notifyDataSetChanged()
    }

    fun actionsFor(result: ModuleResult): List<PanelAction> = buildList {
        fun label(id: Int) = context.getString(id)
        add(PanelAction(label(R.string.ime_action_insert)) { insert(result) })
        add(PanelAction(label(R.string.ime_action_copy)) { activate(result, ::copy) })
        add(PanelAction(label(R.string.ime_action_add_to_queue)) { activate(result, ::enqueue) })
        if (host.field.allowsTransform) {
            add(PanelAction(label(R.string.ime_action_transform)) {
                activate(result) { delivery -> transforms.openText(delivery.text, delivery.secret, surface.title) }
            })
        }
        result.actions.forEach { action -> add(PanelAction(action.title) { runAction(action) }) }
        add(PanelAction(label(R.string.ime_action_open_in_app)) { openApp(context, host) })
    }

    fun insert(result: ModuleResult) = activate(result, ::commit)

    private fun load(query: String) {
        val requested = ++generation
        this.query = query.trim()
        hideForm()
        // Earlier results stay visible until the new ones replace them.
        if (rows.isEmpty()) state?.show(R.drawable.ic_lucide_notebook_text, context.getString(R.string.ime_module_loading, surface.title))
        val arguments = JSONObject().put("query", this.query)
        worker.invokeModule(surface.moduleId, surface.searchCommand, arguments) { output ->
            if (requested != generation) return@invokeModule
            if (output !is ModuleOutput.Results) return@invokeModule showFailed()
            rows.clear()
            rows += output.items
            publish()
        }
    }

    private fun reload() = load(query)

    private fun publish() {
        adapter.notifyDataSetChanged()
        rows.indexOfFirst { it.preselect }.takeIf { it >= 0 }?.let { list?.setSelection(it) }
        val state = state ?: return
        when {
            rows.isNotEmpty() -> state.hide()
            query.isEmpty() -> state.show(
                R.drawable.ic_lucide_notebook_text,
                context.getString(R.string.ime_module_empty, surface.title),
                context.getString(R.string.ime_action_open_in_app),
            ) { openApp(context, host) }
            else -> state.show(R.drawable.ic_lucide_search, context.getString(R.string.ime_no_results, query))
        }
    }

    private fun showFailed() {
        rows.clear()
        adapter.notifyDataSetChanged()
        state?.show(
            R.drawable.ic_lucide_notebook_text,
            context.getString(R.string.ime_module_error, surface.title),
            context.getString(R.string.ime_retry),
        ) { reload() }
    }

    /** Runs the activate command; a form is filled in first, then [then] gets the delivery. */
    private fun activate(result: ModuleResult, then: (ModuleDelivery) -> Unit) {
        val arguments = JSONObject().put("id", result.id)
        worker.invokeModule(surface.moduleId, surface.activateCommand, arguments) { output ->
            handle(output, result.title, result.secret, then)
        }
    }

    private fun handle(output: ModuleOutput, title: String, secret: Boolean, then: (ModuleDelivery) -> Unit) {
        when (output) {
            is ModuleOutput.Deliver -> unlocked(output.delivery.requireAuth) { then(output.delivery) }
            is ModuleOutput.View -> {
                val form = output.form
                // File fields and other views belong to the app.
                if (form != null && form.supported) showForm(form, title, secret, then) else openApp(context, host)
            }
            is ModuleOutput.Message -> {
                host.showNotice(output.text)
                reload()
            }
            else -> failed()
        }
    }

    private fun unlocked(required: Boolean, action: () -> Unit) {
        if (required) host.unlockSecrets(action) else action()
    }

    /** A result action: destructive ones are confirmed first; the list reloads after it ran. */
    private fun runAction(action: ModuleAction) {
        val invoke = {
            unlocked(action.requireAuth) {
                worker.invokeModule(surface.moduleId, action.command, action.arguments) { output ->
                    when (output) {
                        is ModuleOutput.Message -> host.showNotice(output.text)
                        is ModuleOutput.View -> openApp(context, host)
                        is ModuleOutput.Failed -> host.showNotice(context.getString(R.string.ime_module_failed))
                        else -> Unit
                    }
                    reload()
                }
            }
        }
        if (action.destructive) host.showActions(action.title, listOf(PanelAction(action.title, invoke))) else invoke()
    }

    private fun showForm(form: ModuleForm, title: String, secret: Boolean, then: (ModuleDelivery) -> Unit) {
        val slot = formSlot ?: return
        hideForm()
        val previewAction = form.preview
        val created = PanelModuleForm(
            context,
            host,
            title,
            form,
            concealed = secret,
            preview = if (previewAction == null) null else { values, onPreview -> requestPreview(previewAction, values, onPreview) },
            onSubmit = { values ->
                hideForm()
                submit(form.submit, values, title, secret, then)
            },
            onCancel = ::hideForm,
        )
        this.form = created
        slot.addView(created.view)
        slot.visibility = View.VISIBLE
    }

    private fun hideForm() {
        form?.release()
        form = null
        formSlot?.let {
            it.removeAllViews()
            it.visibility = View.GONE
        }
    }

    private fun withValues(action: ModuleAction, values: Map<String, Any>): JSONObject =
        JSONObject(action.arguments.toString()).put("values", ModuleSurfaces.valuesJson(values))

    private fun requestPreview(action: ModuleAction, values: Map<String, Any>, onPreview: (String?) -> Unit) {
        worker.invokeModule(surface.moduleId, action.command, withValues(action, values)) { output ->
            onPreview((output as? ModuleOutput.Message)?.text)
        }
    }

    /** The submit's delivery, or the same form again with the module's errors. */
    private fun submit(action: ModuleAction, values: Map<String, Any>, title: String, secret: Boolean, then: (ModuleDelivery) -> Unit) =
        unlocked(action.requireAuth) {
            worker.invokeModule(surface.moduleId, action.command, withValues(action, values)) { output ->
                handle(output, title, secret, then)
            }
        }

    /** Inserted by commit; the runtime records it when the module asked, never a secret. */
    private fun commit(delivery: ModuleDelivery) {
        val outcome = host.insert.insertText(delivery.text, host.field)
        if (outcome == InsertOutcome.INSERTED && delivery.cursorGraphemesFromEnd > 0) {
            host.insert.moveCaretBack(utf16FromEnd(delivery.text, delivery.cursorGraphemesFromEnd))
        }
        host.afterInsert(outcome) { pasteViaClipboard(delivery.text, delivery.secret) }
        if (delivery.recordHistory && !delivery.secret && !host.field.maskedMode) {
            worker.request("clips", deliverTextOp(delivery, surface.moduleId, "return")) {}
        }
    }

    private fun copy(delivery: ModuleDelivery) {
        if (write(delivery.text, delivery.secret)) host.announce(context.getString(R.string.ime_copied))
    }

    /** Queued as a stored copy that never reaches History; a secret stays masked. */
    private fun enqueue(delivery: ModuleDelivery) {
        queue.enqueue(JSONObject().put("kind", "delivered").put("text", delivery.text).put("secret", delivery.secret))
    }

    /** The explicit fallback: our own clipboard write (never captured), then the editor's paste. */
    private fun pasteViaClipboard(text: String, secret: Boolean) {
        if (write(text, secret) && !host.insert.pasteFromClipboard()) {
            host.showNotice(context.getString(R.string.ime_insert_failed))
        }
    }

    private fun write(text: String, secret: Boolean): Boolean =
        AndroidClipboardWriter.writeText(context, text, "text/plain", secret = secret).also { written ->
            if (!written) host.showNotice(context.getString(R.string.ime_copy_failed))
        }

    private fun failed() {
        host.showNotice(context.getString(R.string.ime_module_failed))
        reload()
    }

    private fun masked(result: ModuleResult): Boolean = result.secret || host.field.maskedMode

    private fun meta(result: ModuleResult): String = (
        result.badges + listOfNotNull(
            if (masked(result)) context.getString(R.string.ime_form_preview_hidden) else result.preview.trim().ifEmpty { null },
        )
    ).joinToString(" · ")

    private fun description(result: ModuleResult, meta: String): String =
        if (result.secret) {
            context.getString(R.string.ime_module_secret_description, result.title)
        } else {
            listOfNotNull(result.title, meta.ifEmpty { null }).joinToString(", ")
        }

    private inner class Adapter : BaseAdapter() {
        override fun getCount(): Int = rows.size
        override fun getItem(position: Int): ModuleResult = rows[position]
        override fun getItemId(position: Int): Long = rows[position].id.hashCode().toLong()
        override fun hasStableIds(): Boolean = true

        override fun getView(position: Int, convertView: View?, parent: ViewGroup): View {
            val row = convertView ?: LayoutInflater.from(context).inflate(R.layout.ime_clip_row, parent, false)
            val result = rows[position]
            val meta = meta(result)
            bindFeatureRow(row, result.title, meta, if (result.secret) R.drawable.ic_lucide_lock else 0, description(result, meta))
            bindRowActions(row, actionsFor(result))
            return row
        }
    }
}

/** UTF-16 units covered by the last [graphemes] user-perceived characters of [text]. */
internal fun utf16FromEnd(text: String, graphemes: Int): Int {
    val breaks = BreakIterator.getCharacterInstance().apply { setText(text) }
    var position = text.length
    repeat(graphemes) {
        val previous = breaks.preceding(position)
        if (previous == BreakIterator.DONE) return text.length
        position = previous
    }
    return text.length - position
}
