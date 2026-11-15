package com.copypaste.app.ime

import android.content.ClipboardManager
import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import com.copypaste.app.AndroidCaptureState
import com.copypaste.app.AndroidClipboardReader
import com.copypaste.app.AndroidClipboardWriter
import com.copypaste.app.BackgroundClipboardMonitor

/** The clipboard as the selected keyboard sees it. */
internal interface PanelClipboard {
    /** The user allows capture while CopyPaste can read the clipboard. */
    fun captureAllowed(): Boolean

    /** Full mode already captures every copy, so the panel must not repeat it. */
    fun coveredByBackgroundCapture(): Boolean

    /**
     * Identity of the current clip: its description timestamp, 0 when the
     * platform has none, `null` when there is no clip or it is CopyPaste's own write.
     */
    fun clipStamp(): Long?

    /** Text of the current clip for the "current clipboard" marker; null for none or sensitive clips. */
    fun currentText(): String?

    fun capture(onSaved: () -> Unit)
    fun listen(onChanged: () -> Unit)
    fun stopListening()
    fun release()
}

/**
 * Panel-mode capture: one catch-up read when the panel opens, and collect
 * mode through the clip-changed callback while CopyPaste stays the selected
 * keyboard. No polling; nothing runs in password or no-learning fields.
 */
internal class PanelCapture(
    private val clipboard: PanelClipboard,
    /** Opens the runtime off the main thread before [run]; immediate in tests. */
    private val whenRuntimeReady: (run: () -> Unit) -> Unit,
    private val onSaved: () -> Unit,
) {
    private var field = FieldPolicy.NONE
    private var lastStamp: Long? = null
    private var listening = false

    fun onFieldChanged(field: FieldPolicy) { this.field = field }

    /** Registers collect mode only while capture is allowed and Full mode is off. */
    fun refreshListening() {
        val wanted = clipboard.captureAllowed() && !clipboard.coveredByBackgroundCapture()
        if (wanted == listening) return
        listening = wanted
        if (wanted) clipboard.listen(::onClipChanged) else clipboard.stopListening()
    }

    /** Reads the current clip once when the panel opens. Returns true if a capture started. */
    fun catchUp(): Boolean = captureIfNew()

    fun onClipChanged() { captureIfNew() }

    fun release() {
        if (listening) clipboard.stopListening()
        listening = false
        clipboard.release()
    }

    private fun captureIfNew(): Boolean {
        val stamp = clipboard.clipStamp() ?: return false
        if (stamp != 0L && stamp == lastStamp) return false
        // A clip seen in a password or private field is never captured later
        // by a catch-up in another field.
        lastStamp = stamp
        if (!field.allowsCapture || !clipboard.captureAllowed() || clipboard.coveredByBackgroundCapture()) {
            return false
        }
        whenRuntimeReady { clipboard.capture(onSaved) }
        return true
    }
}

/** [PanelClipboard] over the shared foreground capture host and its admission. */
internal class NativePanelClipboard(context: Context) : PanelClipboard {
    private val app = context.applicationContext
    private val manager = app.getSystemService(ClipboardManager::class.java)
    private val main = Handler(Looper.getMainLooper())
    private var host: AndroidClipboardReader.Host? = null
    private var listener: ClipboardManager.OnPrimaryClipChangedListener? = null

    override fun captureAllowed(): Boolean = AndroidCaptureState.foregroundCaptureEnabled(app)

    override fun coveredByBackgroundCapture(): Boolean = BackgroundClipboardMonitor.isListening()

    override fun clipStamp(): Long? {
        val description = runCatching { manager?.primaryClipDescription }.getOrNull() ?: return null
        if (AndroidClipboardWriter.isOwned(app, description)) return null
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) description.timestamp else 0L
    }

    override fun currentText(): String? = runCatching {
        val clip = manager?.primaryClip?.takeIf { it.itemCount > 0 } ?: return null
        if (clip.description.extras?.getBoolean(SENSITIVE_EXTRA, false) == true) return null
        clip.getItemAt(0)?.text?.takeIf { it.length <= MAX_MARKER_CHARS }?.toString()
    }.getOrNull()

    override fun capture(onSaved: () -> Unit) {
        host = AndroidClipboardReader.acquireForeground(host)
        AndroidClipboardReader.captureForeground(app, host) { main.post(onSaved) }
    }

    override fun listen(onChanged: () -> Unit) {
        stopListening()
        listener = ClipboardManager.OnPrimaryClipChangedListener { onChanged() }
            .also { runCatching { manager?.addPrimaryClipChangedListener(it) } }
    }

    override fun stopListening() {
        listener?.let { runCatching { manager?.removePrimaryClipChangedListener(it) } }
        listener = null
    }

    override fun release() {
        stopListening()
        AndroidClipboardReader.retireForeground(host)
        host = null
    }

    private companion object {
        const val SENSITIVE_EXTRA = "android.content.extra.IS_SENSITIVE"
        const val MAX_MARKER_CHARS = 64 * 1024
    }
}
