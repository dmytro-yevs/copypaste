package com.copypaste.app

import android.app.Activity
import android.os.Bundle
import android.widget.Toast
import com.copypaste.app.quickaccess.QueueIntake
import com.copypaste.app.quickaccess.QueueIntakeContent
import com.copypaste.app.quickaccess.QueueIntakeOutcome
import com.copypaste.app.quickaccess.QuickAccessRuntime

/**
 * Handles "Add to Paste Queue" from text selection (PROCESS_TEXT) and the
 * share sheet. The selected text is never changed, and the clipboard is
 * never read.
 */
class QueueIntakeActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setResult(RESULT_CANCELED)
        if (savedInstanceState != null) {
            finish()
            return
        }
        val app = applicationContext
        val worker = QuickAccessRuntime.worker(app)
        val report: (QueueIntakeOutcome) -> Unit = { outcome ->
            Toast.makeText(app, outcome.message, Toast.LENGTH_SHORT).show()
            if (!isFinishing) finish()
        }
        when (val content = QueueIntake.parse(intent)) {
            is QueueIntakeContent.Text -> {
                QueueIntake.addText(worker, content, report)
                finish()
            }
            is QueueIntakeContent.Image -> {
                // The shared image's read grant lasts only while this activity does.
                QueueIntake.addImage(this, intent, worker, report)
            }
            null -> report(QueueIntakeOutcome.NOTHING)
        }
    }
}
