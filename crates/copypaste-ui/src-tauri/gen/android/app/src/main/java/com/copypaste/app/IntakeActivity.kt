package com.copypaste.app

import android.app.Activity
import android.content.Intent
import android.os.Bundle

/**
 * Rung 0's text-bearing doorways: the share sheet and text-selection action.
 *
 * Invisible and immediate — it takes the text, hands it to [ClipQueue] and
 * finishes. It is a separate activity from [MainActivity] so that sharing to
 * CopyPaste does not throw the user out of the app they were in.
 *
 */
class IntakeActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val text = fromIntent(intent)
        if (text != null) {
            queueClip(text, sourceOf(intent))
        }
        finish()
    }

    private fun fromIntent(intent: Intent?): String? = when (intent?.action) {
        Intent.ACTION_SEND -> intent.getStringExtra(Intent.EXTRA_TEXT)
        Intent.ACTION_PROCESS_TEXT ->
            intent.getCharSequenceExtra(Intent.EXTRA_PROCESS_TEXT)?.toString()
        else -> null
    }

    private fun sourceOf(intent: Intent?): String =
        if (intent?.action == Intent.ACTION_PROCESS_TEXT) "process_text" else "share"
}
