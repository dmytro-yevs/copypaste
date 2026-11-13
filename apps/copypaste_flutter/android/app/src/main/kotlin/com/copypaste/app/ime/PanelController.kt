package com.copypaste.app.ime

import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.ResultReceiver
import android.text.Html
import android.util.Base64
import android.view.View
import android.view.inputmethod.InputConnection
import androidx.core.content.FileProvider
import com.copypaste.app.AndroidClipboardCache
import com.copypaste.app.AndroidClipboardWriter
import com.copypaste.app.DeviceAuthenticator
import com.copypaste.app.QuickAccessActivity
import com.copypaste.app.R
import com.copypaste.app.RuntimeAccessWorker
import com.copypaste.app.RuntimeErrorCode
import com.copypaste.app.RuntimeReply
import com.copypaste.app.ScreenshotProtection
import java.io.File
import java.util.UUID
import kotlin.math.min
import kotlin.math.roundToInt
import org.json.JSONObject

/** What the panel needs from its input method service. */
internal interface PanelHost {
    fun inputConnection(): InputConnection?
    fun switchBack()
    fun startActivity(intent: Intent)
    fun setSecureWindow(secure: Boolean)
}

/** Panel keypad keys routed to a feature tab's own field instead of the search. */
internal class KeypadTarget(
    val onText: (String) -> Unit,
    val onBackspace: () -> Unit,
    val onClosed: () -> Unit,
)

/** What a feature tab (snippets, queue, transforms) may ask of the panel. */
internal interface PanelTabHost {
    val field: FieldPolicy
    val insert: PanelInsert
    fun inputConnection(): InputConnection?
    fun clipboardText(): String?
    fun showActions(title: String, actions: List<PanelAction>)
    fun showNotice(text: String, actionLabel: String? = null, action: (() -> Unit)? = null)
    fun announce(text: String)
    fun unlockSecrets(action: () -> Unit)
    /** Announces, offers [pasteViaClipboard] after a refused commit, and auto-returns. */
    fun afterInsert(outcome: InsertOutcome, pasteViaClipboard: () -> Unit)
    fun openKeypad(target: KeypadTarget)
    fun closeKeypad()
    fun selectTab(tab: PanelTab)
    fun queueChanged()
    fun startActivity(intent: Intent)
    /** Inserts a history item as it is, bypassing the default transform. */
    fun insertOriginal(item: PanelItem)
}

/** The default transform applied to history text inserts (per app, or global). */
internal interface InsertDefault {
    /** Resolves the default for [field]; [onResolved] gets its chip label, or null when none applies. */
    fun resolve(field: FieldPolicy, onResolved: (String?) -> Unit)
    /** Runs the resolved default on [item]; [then] gets the result, or null to insert the original. */
    fun apply(item: PanelItem, then: (String?) -> Unit)
}

/**
 * Decides what the keyboard panel shows and does. Runtime calls go through
 * the blocking facade on its worker thread; replies arrive on the main thread.
 */
internal class PanelController(
    private val context: Context,
    private val host: PanelHost,
    private val worker: RuntimeAccessWorker,
    private val clipboard: PanelClipboard,
    private val preferences: PanelPreferences,
    private val unlock: SecretUnlock = SecretUnlock(),
    private val locked: () -> Boolean = { deviceLocked(context) },
    private val authenticationAvailable: () -> Boolean = {
        DeviceAuthenticator.availability(context) == DeviceAuthenticator.Availability.Available
    },
    private val screenshotsBlocked: () -> Boolean = { ScreenshotProtection.blocked },
    clock: () -> Long = System::currentTimeMillis,
) : PanelViewEvents, PanelTabHost {
    private val main = Handler(Looper.getMainLooper())
    override val insert = PanelInsert(host::inputConnection)
    val history = PanelHistoryList(context, worker, ::onListState, clock)
    private val capture = PanelCapture(clipboard, whenRuntimeReady = ::whenRuntimeReady, onSaved = ::onCaptured)
    private val tabContents = linkedMapOf<PanelTab, PanelTabContent>()
    private val itemActions = mutableListOf<(PanelItem) -> PanelAction?>()
    private var insertDefault: InsertDefault? = null
    private var defaultLabel: String? = null
    private var view: PanelView? = null
    private var keypad: PanelKeypad? = null
    private var keypadTarget: KeypadTarget? = null
    override var field = FieldPolicy.NONE
        private set
    private var shown = false
    private var runtimeReady = false
    private var tab = PanelTab.HISTORY
    private var query = ""
    private var queueCount = 0
    private var autoReturnSuspended = false
    private var pendingAuthentication: (() -> Unit)? = null
    private var authorized: (() -> Unit)? = null
    private val runSearch = Runnable { search() }
    private val authenticationReceiver = object : ResultReceiver(main) {
        override fun onReceiveResult(resultCode: Int, resultData: Bundle?) =
            onAuthenticated(QuickAccessActivity.outcome(resultCode))
    }

    init {
        history.actionsFor = ::actionsFor
    }

    val hasView: Boolean get() = view != null

    /** A tab body from another package (snippets, queue). */
    fun registerTab(content: PanelTabContent) { tabContents[content.tab] = content }

    /** An extra item action from another package (transform, add to queue). */
    fun registerItemAction(action: (PanelItem) -> PanelAction?) { itemActions += action }

    fun registerInsertDefault(default: InsertDefault) { insertDefault = default }

    fun onCreate() = capture.refreshListening()

    fun createView(): View {
        val panel = PanelView(context, this)
        panel.list.adapter = history.adapter
        applyHeight(panel)
        view = panel
        return panel.root
    }

    /** Drops the view hierarchy while the panel is hidden; returns an empty placeholder. */
    fun releaseView(): View {
        view = null
        keypad = null
        return View(context)
    }

    fun onStartInput(field: FieldPolicy) {
        this.field = field
        capture.onFieldChanged(field)
    }

    fun onShow(field: FieldPolicy) {
        shown = true
        onStartInput(field)
        capture.refreshListening()
        val view = view ?: return
        view.hideActions()
        closeKeypad()
        applySecureWindow()
        if (locked()) {
            showLocked()
            return
        }
        history.maskedMode = field.maskedMode
        history.imagesAccepted = field.accepts(IMAGE_PNG)
        tab = preferences.lastTab.takeIf { it in availableTabs() } ?: PanelTab.HISTORY
        view.setTabs(availableTabs(), tab)
        view.setQuery(query, editing = false)
        view.setUndoEnabled(insert.canUndo)
        view.setQueueCount(queueCount)
        showFieldNotice()
        showTab()
        insertDefault?.resolve(field) { label ->
            defaultLabel = label
            if (shown && !locked()) showFieldNotice()
        }
        refreshQueue {
            if (!shown) return@refreshQueue
            capture.catchUp()
            if (field.maskedMode) return@refreshQueue
            history.currentClipText = clipboard.currentText()
            history.adapter.notifyDataSetChanged()
        }
        authorized?.let {
            authorized = null
            it()
        }
    }

    fun onHide() {
        shown = false
        main.removeCallbacks(runSearch)
        unlock.onPanelHidden()
        if (!unlock.authenticating) history.secretsRevealed = false
        history.release()
        tabContents.values.forEach(PanelTabContent::release)
        view?.let {
            it.hideTabContent()
            it.hideActions()
            it.setNotice(null)
        }
        closeKeypad()
        autoReturnSuspended = false
        defaultLabel = null
        query = ""
        insert.forget()
        applySecureWindow()
    }

    fun onDestroy() {
        onHide()
        capture.release()
        view = null
    }

    fun onSelectionChanged(start: Int, end: Int) {
        insert.onSelectionChanged(start, end)
        view?.setUndoEnabled(insert.canUndo)
    }

    /** Screen off or keyguard: no content stays on screen. */
    fun onScreenLocked() {
        unlock.lock()
        pendingAuthentication = null
        authorized = null
        history.secretsRevealed = false
        if (shown) showLocked()
        applySecureWindow()
    }

    fun onUserUnlocked() {
        if (shown) onShow(field)
    }

    /** Hardware Escape: closes the sheet or keypad first, then switches back. */
    fun onEscape() {
        val view = view
        when {
            view?.actionsShown == true -> view.hideActions()
            keypad != null -> closeKeypad()
            else -> host.switchBack()
        }
    }

    internal fun onAuthenticated(outcome: DeviceAuthenticator.Outcome) {
        val success = outcome == DeviceAuthenticator.Outcome.Success
        unlock.finishAuthentication(success)
        val action = pendingAuthentication
        pendingAuthentication = null
        applySecureWindow()
        if (!success) {
            if (shown) showNotice(R.string.ime_unlock_failed)
            return
        }
        if (action == null) return
        if (shown && host.inputConnection() != null) action() else authorized = action
    }

    override fun onSearchTapped() {
        if (keypad != null && keypadTarget == null) return closeKeypad()
        showKeypad(null)
    }

    override fun onSwitchBack() = host.switchBack()

    override fun inputConnection(): InputConnection? = host.inputConnection()

    override fun clipboardText(): String? = clipboard.currentText()

    override fun showActions(title: String, actions: List<PanelAction>) {
        autoReturnSuspended = true
        view?.showActions(title, actions)
    }

    override fun showNotice(text: String, actionLabel: String?, action: (() -> Unit)?) {
        if (!shown) return
        view?.setNotice(text, actionLabel) {
            showFieldNotice()
            action?.invoke()
        }
    }

    override fun announce(text: String) {
        view?.announce(text)
    }

    override fun unlockSecrets(action: () -> Unit) = authenticate(action)

    override fun openKeypad(target: KeypadTarget) = showKeypad(target)

    override fun selectTab(tab: PanelTab) = if (tab == this.tab) showTab() else onTabSelected(tab)

    override fun queueChanged() = refreshQueue()

    override fun startActivity(intent: Intent) = host.startActivity(intent)

    override fun insertOriginal(item: PanelItem) = withSecret(item) { insertText(item, plain = false, original = true) }

    override fun onTabSelected(tab: PanelTab) {
        if (tab == this.tab) return
        this.tab = tab
        preferences.lastTab = tab
        if (tab == PanelTab.QUEUE) autoReturnSuspended = true
        view?.setTabs(availableTabs(), tab)
        showTab()
    }

    override fun onBackspace() {
        insert.backspace()
        view?.setUndoEnabled(false)
    }

    override fun onEnter() {
        insert.enter(field)
        view?.setUndoEnabled(false)
    }

    override fun onUndo() {
        insert.undo()
        view?.setUndoEnabled(false)
    }

    override fun onQueueIndicator() {
        if (PanelTab.QUEUE in tabContents) onTabSelected(PanelTab.QUEUE)
    }

    override fun onItemTapped(item: PanelItem) {
        showFieldNotice()
        when {
            item.isImage -> insertImage(item)
            !item.isText -> showNotice(R.string.ime_file_not_accepted, R.string.ime_action_copy) { copy(item, paste = false) }
            else -> withSecret(item) { insertText(item, plain = false) }
        }
    }

    override fun onItemLongPressed(item: PanelItem) {
        autoReturnSuspended = true
        view?.showActions(history.summary(item), actionsFor(item))
    }

    override fun onResized(heightDp: Int) {
        preferences.panelHeightDp = heightDp
    }

    fun actionsFor(item: PanelItem): List<PanelAction> = buildList {
        fun label(id: Int) = context.getString(id)
        if (item.isText || (item.isImage && field.accepts(IMAGE_PNG))) {
            add(PanelAction(label(R.string.ime_action_insert)) { onItemTapped(item) })
        }
        if (item.isText && !item.contentType.startsWith("text/plain")) {
            add(PanelAction(label(R.string.ime_action_insert_plain)) { withSecret(item) { insertText(item, plain = true) } })
        }
        add(PanelAction(label(R.string.ime_action_copy)) { copy(item, paste = false) })
        add(PanelAction(label(if (item.pinned) R.string.ime_action_unpin else R.string.ime_action_pin)) { togglePin(item) })
        add(PanelAction(label(R.string.ime_action_delete)) { confirmDelete(item) })
        if (item.visibility.masked && !field.maskedMode && !history.secretsRevealed) {
            add(PanelAction(label(R.string.ime_action_reveal)) { reveal() })
        }
        itemActions.mapNotNullTo(this) { it(item) }
    }

    private fun availableTabs(): List<PanelTab> =
        PanelTab.entries.filter { it == PanelTab.HISTORY || it == PanelTab.PINNED || it in tabContents }

    private fun showTab() {
        val view = view ?: return
        val content = tabContents[tab]
        if (content != null) {
            history.release()
            view.showTabContent(content.createView(view.body))
            content.onShown(query)
        } else {
            view.hideTabContent()
            history.load(query, pinnedOnly = tab == PanelTab.PINNED)
        }
    }

    private fun showLocked() {
        history.release()
        tabContents.values.forEach(PanelTabContent::release)
        view?.let {
            it.hideActions()
            it.setTabs(emptyList(), PanelTab.HISTORY)
            it.setNotice(null)
            it.setQueueCount(0)
            it.setListState(PanelListState.Locked)
            it.setStateAction(null, null)
        }
        closeKeypad()
    }

    private fun onListState(state: PanelListState) {
        val view = view ?: return
        if (!shown || tab in tabContents) return
        view.setListState(state)
        if (state == PanelListState.Failed) {
            view.setStateAction(context.getString(R.string.ime_retry)) { history.reload() }
        } else {
            view.setStateAction(null, null)
        }
    }

    private fun onQueryEdited() {
        view?.setQuery(query, editing = true)
        main.removeCallbacks(runSearch)
        main.postDelayed(runSearch, SEARCH_DEBOUNCE_MS)
    }

    private fun search() {
        val content = tabContents[tab]
        if (content != null) content.onQueryChanged(query) else history.load(query, tab == PanelTab.PINNED)
    }

    /** A keypad for the search, or for [target]'s field. */
    private fun showKeypad(target: KeypadTarget?) {
        val view = view ?: return
        closeKeypad()
        keypadTarget = target
        val pad = PanelKeypad(
            context,
            KeypadLayout.available(KeypadLayout.deviceLocales()),
            preferences.keypadLayout,
            onText = { text ->
                if (target != null) {
                    target.onText(text)
                } else {
                    query += text
                    onQueryEdited()
                }
            },
            onBackspace = {
                if (target != null) {
                    target.onBackspace()
                } else {
                    if (query.isNotEmpty()) query = query.substring(0, query.offsetByCodePoints(query.length, -1))
                    onQueryEdited()
                }
            },
            onDone = ::closeKeypad,
            onLayoutChanged = { preferences.keypadLayout = it.key },
        )
        keypad = pad
        view.showKeypad(pad)
        view.setTabsVisible(false)
        if (target == null) view.setQuery(query, editing = true)
    }

    override fun closeKeypad() {
        keypad?.release()
        keypad = null
        keypadTarget?.let {
            keypadTarget = null
            it.onClosed()
        }
        view?.let {
            it.showKeypad(null)
            it.setTabsVisible(true)
            it.setQuery(query, editing = false)
        }
    }

    private fun insertText(item: PanelItem, plain: Boolean, original: Boolean = false) {
        val default = insertDefault?.takeIf { !plain && !original && defaultLabel != null }
        if (default != null) {
            default.apply(item) { text ->
                if (text != null) onInserted(insert.insertText(text, field), item) else insertText(item, plain, original = true)
            }
            return
        }
        worker.request("get", JSONObject().put("id", item.id)) { reply ->
            val content = itemContent(reply, item) ?: return@request
            val text: CharSequence = if (item.isHtml) {
                Html.fromHtml(content, Html.FROM_HTML_MODE_LEGACY).let { if (plain) it.toString() else it }
            } else {
                content
            }
            onInserted(insert.insertText(text, field), item)
        }
    }

    private fun insertImage(item: PanelItem) {
        if (!field.accepts(IMAGE_PNG) || Build.VERSION.SDK_INT < Build.VERSION_CODES.N_MR1) {
            showNotice(R.string.ime_image_not_accepted, R.string.ime_action_copy) { copy(item, paste = false) }
            return
        }
        withSecret(item) {
            worker.request("image_preview", JSONObject().put("id", item.id).put("max_edge", IMAGE_COMMIT_EDGE)) { reply ->
                val encoded = (reply as? RuntimeReply.Ok)?.objectData()?.optString("png_base64").orEmpty()
                val uri = runCatching { stageImage(Base64.decode(encoded, Base64.DEFAULT)) }.getOrNull()
                if (encoded.isEmpty() || uri == null) {
                    onInserted(InsertOutcome.REJECTED, item)
                } else {
                    onInserted(insert.insertContent(uri, IMAGE_PNG, kindLabel(context, item), field), item)
                }
            }
        }
    }

    /** Stages committed image bytes like other clipboard payloads, so the cache cleanup owns them. */
    private fun stageImage(bytes: ByteArray): android.net.Uri {
        require(bytes.isNotEmpty())
        val directory = File(context.cacheDir, "clipboard/${UUID.randomUUID()}").apply { mkdirs() }
        val file = File(directory, "image.png").apply { writeBytes(bytes) }
        AndroidClipboardCache.scheduleCleanup(context)
        return FileProvider.getUriForFile(context, "${context.packageName}.clipboard", file)
    }

    private fun onInserted(outcome: InsertOutcome, item: PanelItem) {
        if (outcome == InsertOutcome.UNSUPPORTED) {
            showNotice(R.string.ime_image_not_accepted, R.string.ime_action_copy) { copy(item, paste = false) }
        } else {
            afterInsert(outcome) { copy(item, paste = true) }
        }
    }

    override fun afterInsert(outcome: InsertOutcome, pasteViaClipboard: () -> Unit) {
        when (outcome) {
            InsertOutcome.INSERTED -> {
                view?.announce(context.getString(R.string.ime_inserted))
                view?.setUndoEnabled(insert.canUndo)
                if (preferences.autoReturn && !autoReturnSuspended && queueCount == 0 && tab != PanelTab.QUEUE) {
                    host.switchBack()
                }
            }
            InsertOutcome.REJECTED ->
                showNotice(R.string.ime_insert_failed, R.string.ime_paste_via_clipboard, pasteViaClipboard)
            InsertOutcome.TOO_LARGE ->
                showNotice(R.string.ime_insert_too_large, R.string.ime_paste_via_clipboard, pasteViaClipboard)
            InsertOutcome.UNSUPPORTED -> showNotice(R.string.ime_image_not_accepted)
        }
    }

    /**
     * Puts the clip on the clipboard. In password and private fields the
     * text goes straight to the writer so no history row is promoted.
     */
    private fun copy(item: PanelItem, paste: Boolean) = withSecret(item) {
        val done: (Boolean) -> Unit = { written ->
            if (!written) {
                showNotice(R.string.ime_copy_failed)
            } else if (!paste) {
                view?.announce(context.getString(R.string.ime_copied))
            } else if (!insert.pasteFromClipboard()) {
                showNotice(R.string.ime_insert_failed)
            }
        }
        if (field.recordsUsage || !item.isText) {
            worker.request("copy", JSONObject().put("id", item.id)) { done(it is RuntimeReply.Ok) }
        } else {
            worker.request("get", JSONObject().put("id", item.id)) { reply ->
                val content = itemContent(reply, item) ?: return@request
                done(AndroidClipboardWriter.writeText(context, content, item.contentType, secret = item.visibility.masked))
            }
        }
    }

    private fun togglePin(item: PanelItem) {
        worker.request("pin", JSONObject().put("id", item.id).put("pinned", !item.pinned)) { history.reload() }
    }

    private fun confirmDelete(item: PanelItem) {
        view?.showActions(
            context.getString(R.string.ime_delete_confirm),
            listOf(
                PanelAction(context.getString(R.string.ime_action_delete)) {
                    worker.request("delete", JSONObject().put("id", item.id)) { reply ->
                        if (reply is RuntimeReply.Ok) history.remove(item.id) else showNotice(R.string.ime_delete_failed)
                    }
                },
            ),
        )
    }

    private fun reveal() = authenticate {
        history.secretsRevealed = true
        history.adapter.notifyDataSetChanged()
        applySecureWindow()
    }

    /** Full item content, or a notice when the clip is gone (deleted elsewhere) or unreadable. */
    private fun itemContent(reply: RuntimeReply, item: PanelItem): String? {
        val content = (reply as? RuntimeReply.Ok)?.objectData()?.optString("content")
        if (content != null) return content
        if ((reply as? RuntimeReply.Failure)?.code == RuntimeErrorCode.NOT_FOUND) {
            history.remove(item.id)
            showNotice(R.string.ime_item_unavailable)
        } else {
            showNotice(R.string.ime_error)
        }
        return null
    }

    private fun withSecret(item: PanelItem, action: () -> Unit) {
        if (item.visibility.masked) authenticate(action) else action()
    }

    private fun authenticate(action: () -> Unit) {
        if (unlock.unlocked) return action()
        if (!authenticationAvailable()) {
            // Without a secure lock screen there is nothing to verify; the tap is the consent.
            unlock.finishAuthentication(true)
            applySecureWindow()
            return action()
        }
        if (unlock.authenticating) return
        pendingAuthentication = action
        unlock.beginAuthentication()
        val reason = context.getString(R.string.device_auth_reason_unlock_secrets)
        host.startActivity(QuickAccessActivity.authenticationIntent(context, reason, authenticationReceiver))
    }

    private fun whenRuntimeReady(run: () -> Unit) {
        if (runtimeReady) run() else refreshQueue(run)
    }

    private fun refreshQueue(then: (() -> Unit)? = null) {
        worker.request("paste_queue", JSONObject().put("op", JSONObject().put("op", "get"))) { reply ->
            // Any reply means the transport opened the runtime.
            runtimeReady = true
            queueCount = (reply as? RuntimeReply.Ok)?.let { pendingQueueEntries(it.objectData()) } ?: 0
            if (shown && !locked()) view?.setQueueCount(queueCount)
            then?.invoke()
        }
    }

    private fun onCaptured() {
        refreshQueue()
        if (!shown || tab in tabContents) return
        if (!field.maskedMode) history.currentClipText = clipboard.currentText()
        history.reload()
    }

    private fun showFieldNotice() {
        val text = when {
            field.maskedMode -> R.string.ime_masked_mode
            field.incognito -> R.string.ime_private_field
            else -> null
        }
        view?.setNotice(listOfNotNull(text?.let(context::getString), defaultLabel).joinToString(" · ").ifEmpty { null })
    }

    private fun showNotice(text: Int, action: Int? = null, run: (() -> Unit)? = null) =
        showNotice(context.getString(text), action?.let(context::getString), run)

    private fun applySecureWindow() {
        host.setSecureWindow(screenshotsBlocked() || field.maskedMode || unlock.unlocked || history.secretsRevealed)
    }

    private fun applyHeight(panel: PanelView) {
        val metrics = context.resources.displayMetrics
        val minimum = (MIN_BODY_DP * metrics.density).roundToInt()
        val maximum = (metrics.heightPixels * MAX_BODY_SCREEN_SHARE).roundToInt().coerceAtLeast(minimum)
        val stored = (preferences.panelHeightDp * metrics.density).roundToInt()
        val default = min((DEFAULT_BODY_DP * metrics.density).roundToInt(), (metrics.heightPixels * DEFAULT_BODY_SCREEN_SHARE).roundToInt())
        panel.bodyHeightRange = minimum..maximum
        panel.setBodyHeight((if (stored > 0) stored else default).coerceIn(minimum, maximum))
    }

    companion object {
        const val IMAGE_PNG = "image/png"
        private const val IMAGE_COMMIT_EDGE = 2_048
        private const val SEARCH_DEBOUNCE_MS = 150L
        private const val MIN_BODY_DP = 96
        private const val DEFAULT_BODY_DP = 200
        private const val DEFAULT_BODY_SCREEN_SHARE = 0.3
        private const val MAX_BODY_SCREEN_SHARE = 0.6
    }
}
