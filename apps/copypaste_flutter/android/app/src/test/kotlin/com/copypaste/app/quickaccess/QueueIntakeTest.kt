package com.copypaste.app.quickaccess

import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.net.Uri
import org.robolectric.RuntimeEnvironment
import com.copypaste.app.QueueIntakeActivity
import com.copypaste.app.RuntimeAccessWorker
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class QueueIntakeTest {
    private val context = RuntimeEnvironment.getApplication()

    @After fun restoreRuntime() {
        QuickAccessRuntime.factory = RuntimeAccessWorker::native
    }

    @Test fun selectionAndShareAreReadFromTheIntent() {
        val selection = Intent(Intent.ACTION_PROCESS_TEXT).putExtra(Intent.EXTRA_PROCESS_TEXT, "picked")
        val shared = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, "sent")
        val image = Uri.parse("content://media/external/images/1")
        val picture = Intent(Intent.ACTION_SEND).setType("image/png").putExtra(Intent.EXTRA_STREAM, image)

        assertEquals(QueueIntakeContent.Text("picked", QueueIntake.VIA_SELECTION), QueueIntake.parse(selection))
        assertEquals(QueueIntakeContent.Text("sent", QueueIntake.VIA_SHARE), QueueIntake.parse(shared))
        assertEquals(QueueIntakeContent.Image(image), QueueIntake.parse(picture))
        assertNull(QueueIntake.parse(Intent(Intent.ACTION_PROCESS_TEXT).putExtra(Intent.EXTRA_PROCESS_TEXT, "  ")))
    }

    @Test fun selectionActionAppendsTheSelectionWithoutTheClipboard() {
        val clipboard = context.getSystemService(ClipboardManager::class.java)
        clipboard.setPrimaryClip(ClipData.newPlainText("", "on the clipboard"))
        val runtime = FakeQuickAccessRuntime { method, _ ->
            if (method == "paste_queue") JSONObject().put("paste_queue", JSONObject()) else null
        }.install()
        val intent = Intent(context, QueueIntakeActivity::class.java)
            .setAction(Intent.ACTION_PROCESS_TEXT)
            .putExtra(Intent.EXTRA_PROCESS_TEXT, "selected words")

        val activity = Robolectric.buildActivity(QueueIntakeActivity::class.java, intent).setup().get()

        assertEquals(listOf("paste_queue"), runtime.methods())
        val op = runtime.requests.single().getJSONObject("params").getJSONObject("op")
        assertEquals("add", op.getString("op"))
        val source = op.getJSONArray("sources").getJSONObject(0)
        assertEquals("text", source.getString("kind"))
        assertEquals("selected words", source.getString("text"))
        assertEquals(QueueIntake.VIA_SELECTION, source.getString("via"))
        assertEquals("on the clipboard", clipboard.primaryClip?.getItemAt(0)?.text)
        assertTrue(activity.isFinishing)
        assertEquals(Activity.RESULT_CANCELED, shadowOf(activity).resultCode)
    }
}
