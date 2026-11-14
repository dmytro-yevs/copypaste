package com.copypaste.app.ime

import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.widget.EditText
import android.widget.TextView
import com.copypaste.app.R
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class PanelInsertTest {
    private val activity = Robolectric.buildActivity(Activity::class.java).setup().get()

    @Test fun insertReplacesTheSelectionWithoutTouchingTheClipboardAndUndoRestoresIt() {
        val clipboard = activity.getSystemService(ClipboardManager::class.java)
        clipboard.setPrimaryClip(ClipData.newPlainText("user", "copied earlier"))
        val editor = EditText(activity).apply {
            setText("Hello world")
            activity.setContentView(this)
            setSelection(6, 11)
        }
        val info = EditorInfo()
        val connection = editor.onCreateInputConnection(info)
        val insert = PanelInsert { connection }
        insert.onSelectionChanged(6, 11)

        assertEquals(InsertOutcome.INSERTED, insert.insertText("there", FieldPolicy.of(info)))
        insert.onSelectionChanged(11, 11)

        assertEquals("Hello there", editor.text.toString())
        assertEquals("copied earlier", clipboard.primaryClip!!.getItemAt(0).text.toString())
        assertTrue(insert.canUndo)
        assertTrue(insert.undo())
        assertEquals("Hello world", editor.text.toString())
    }

    @Test fun undoIsWithdrawnOnceTheCaretMoves() {
        val editor = EditText(activity).apply { activity.setContentView(this) }
        val info = EditorInfo()
        val connection = editor.onCreateInputConnection(info)
        val insert = PanelInsert { connection }
        insert.onSelectionChanged(0, 0)
        insert.insertText("abc", FieldPolicy.of(info))
        insert.onSelectionChanged(3, 3)

        insert.onSelectionChanged(1, 1)

        assertFalse(insert.canUndo)
        assertFalse(insert.undo())
        assertEquals("abc", editor.text.toString())
    }

    @Test fun oversizedTextIsRefusedWhole() {
        val editor = EditText(activity).apply { activity.setContentView(this) }
        val info = EditorInfo()
        val connection = editor.onCreateInputConnection(info)

        val outcome = PanelInsert { connection }.insertText("x".repeat(PanelInsert.MAX_COMMIT_CHARS + 1), FieldPolicy.of(info))

        assertEquals(InsertOutcome.TOO_LARGE, outcome)
        assertEquals("", editor.text.toString())
    }

    @Test fun aRejectedCommitOffersPasteViaClipboard() {
        val rejecting = object : BaseInputConnection(TextView(activity), true) {
            var pasted = false
            override fun commitText(text: CharSequence?, newCursorPosition: Int): Boolean = false
            override fun performContextMenuAction(id: Int): Boolean {
                pasted = id == android.R.id.paste
                return true
            }
        }
        val runtime = FakeRuntime(listOf(FakeRuntime.item("a", "alpha")))
        val panel = controller(activity, runtime, FakeHost(rejecting))
        val root = panel.createView()
        panel.onShow(FieldPolicy(textEditor = true))

        panel.onItemTapped(panel.history.loaded.single())

        val notice = root.findViewById<TextView>(R.id.ime_notice_text)
        val action = root.findViewById<TextView>(R.id.ime_notice_action)
        assertEquals(activity.getString(R.string.ime_insert_failed), notice.text.toString())
        assertEquals(activity.getString(R.string.ime_paste_via_clipboard), action.text.toString())
        assertFalse("copy" in runtime.methods)

        action.performClick()

        assertTrue("copy" in runtime.methods)
        assertTrue(rejecting.pasted)
    }
}
