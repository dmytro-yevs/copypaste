package com.copypaste.app.ime

import android.content.ClipDescription
import android.net.Uri
import android.os.Build
import android.os.SystemClock
import android.view.KeyCharacterMap
import android.view.KeyEvent
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputContentInfo

internal enum class InsertOutcome { INSERTED, REJECTED, TOO_LARGE, UNSUPPORTED }

/**
 * Host-field edits through the input connection. Inserting never touches the
 * system clipboard; only [pasteFromClipboard] asks the editor to paste.
 */
internal class PanelInsert(private val connection: () -> InputConnection?) {
    private data class Committed(val text: String, val replaced: String, val end: Int)

    private var last: Committed? = null
    private var selectionStart = -1
    private var selectionEnd = -1

    /** True while the caret still sits right after the last insert. */
    val canUndo: Boolean get() = last?.let(::canUndoRecord) == true

    fun onSelectionChanged(start: Int, end: Int) {
        selectionStart = start
        selectionEnd = end
        last?.let { if (!canUndoRecord(it)) last = null }
    }

    /** Replaces the selection with [text] in full, or reports why it could not. */
    fun insertText(text: CharSequence, field: FieldPolicy): InsertOutcome {
        if (text.length > MAX_COMMIT_CHARS) return InsertOutcome.TOO_LARGE
        if (!field.textEditor) return InsertOutcome.REJECTED
        val ic = connection() ?: return InsertOutcome.REJECTED
        val replaced = ic.getSelectedText(0)?.toString().orEmpty()
        val start = if (selectionStart >= 0 && selectionEnd >= 0) minOf(selectionStart, selectionEnd) else -1
        ic.beginBatchEdit()
        val committed = try {
            ic.commitText(text, 1)
        } finally {
            ic.endBatchEdit()
        }
        if (!committed) {
            last = null
            return InsertOutcome.REJECTED
        }
        last = Committed(text.toString(), replaced, if (start >= 0) start + text.length else -1)
        return InsertOutcome.INSERTED
    }

    /** Commits [uri] as rich content when the editor declared [mimeType]. */
    fun insertContent(uri: Uri, mimeType: String, label: String, field: FieldPolicy): InsertOutcome {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.N_MR1 || !field.accepts(mimeType)) {
            return InsertOutcome.UNSUPPORTED
        }
        val ic = connection() ?: return InsertOutcome.REJECTED
        last = null
        val info = InputContentInfo(uri, ClipDescription(label, arrayOf(mimeType)))
        return if (ic.commitContent(info, InputConnection.INPUT_CONTENT_GRANT_READ_URI_PERMISSION, null)) {
            InsertOutcome.INSERTED
        } else {
            InsertOutcome.REJECTED
        }
    }

    /** Removes exactly the last insert and restores the text it replaced. */
    fun undo(): Boolean {
        val record = last ?: return false
        last = null
        if (!canUndoRecord(record)) return false
        val ic = connection() ?: return false
        if (ic.getTextBeforeCursor(record.text.length, 0)?.toString() != record.text) return false
        ic.beginBatchEdit()
        try {
            ic.deleteSurroundingText(record.text.length, 0)
            if (record.replaced.isNotEmpty()) ic.commitText(record.replaced, 1)
        } finally {
            ic.endBatchEdit()
        }
        return true
    }

    fun backspace() {
        last = null
        sendKey(KeyEvent.KEYCODE_DEL)
    }

    /** Runs the editor's action (Send, Search, Next, Done), or a newline. */
    fun enter(field: FieldPolicy) {
        last = null
        val action = field.imeAction
        val hasAction = !field.multiLine && !field.noEnterAction &&
            action != EditorInfo.IME_ACTION_NONE && action != EditorInfo.IME_ACTION_UNSPECIFIED
        if (hasAction) connection()?.performEditorAction(action) else sendKey(KeyEvent.KEYCODE_ENTER)
    }

    fun pasteFromClipboard(): Boolean {
        last = null
        return connection()?.performContextMenuAction(android.R.id.paste) == true
    }

    fun forget() {
        last = null
        selectionStart = -1
        selectionEnd = -1
    }

    private fun canUndoRecord(record: Committed): Boolean =
        record.end < 0 || (selectionStart == selectionEnd && selectionEnd == record.end)

    private fun sendKey(code: Int) {
        val ic = connection() ?: return
        val now = SystemClock.uptimeMillis()
        val flags = KeyEvent.FLAG_SOFT_KEYBOARD or KeyEvent.FLAG_KEEP_TOUCH_MODE
        ic.sendKeyEvent(KeyEvent(now, now, KeyEvent.ACTION_DOWN, code, 0, 0, KeyCharacterMap.VIRTUAL_KEYBOARD, 0, flags))
        ic.sendKeyEvent(KeyEvent(now, now, KeyEvent.ACTION_UP, code, 0, 0, KeyCharacterMap.VIRTUAL_KEYBOARD, 0, flags))
    }

    companion object {
        /**
         * Larger text is refused rather than risking a partial commit across
         * the binder. Provisional until device measurement sets the threshold.
         */
        const val MAX_COMMIT_CHARS = 200_000
    }
}
