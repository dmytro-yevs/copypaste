package com.copypaste.app

import android.app.Activity
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Bundle

/**
 * Rung 0's three doorways: the share sheet, the text-selection action, and the
 * Quick Settings tile.
 *
 * Invisible and immediate — it takes the text, hands it to [ClipQueue] and
 * finishes. It is a separate activity from [MainActivity] so that sharing to
 * CopyPaste does not throw the user out of the app they were in.
 *
 * ## Why the tile needs an activity at all
 *
 * Android 10 removed background clipboard reads. The exemption we can reach
 * without a permission is window focus: an app in front may read the clipboard.
 * Tapping the tile launches this, this takes focus, and the read is legal. That
 * is the whole mechanism — no overlay, no logcat, no accessibility service.
 *
 * The read happens in [onWindowFocusChanged] rather than [onCreate] because
 * focus arrives after the first layout pass, and reading before it returns
 * null.
 */
class IntakeActivity : Activity() {
    private var handled = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val text = fromIntent(intent)
        if (text != null) {
            accept(text, sourceOf(intent))
            finish()
        }
        // Otherwise this is a tile tap: wait for focus.
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (!hasFocus || handled) return
        handled = true
        val clipboard = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val text = clipboard.primaryClip
            ?.takeIf { it.itemCount > 0 }
            ?.getItemAt(0)
            ?.coerceToText(this)
            ?.toString()
        if (!text.isNullOrBlank()) accept(text, "tile")
        finish()
    }

    /**
     * Hand it over, and make sure something is there to take it.
     *
     * [ClipQueue] is in memory, so a clip captured while the app is not running
     * is lost if the process dies before Rust drains it. Bringing the app up is
     * the cheap fix, and it doubles as the confirmation that something was
     * saved.
     */
    private fun accept(text: String, source: String) {
        ClipQueue.offer(text, source)
        if (!ClipQueue.rustIsUp) {
            startActivity(
                Intent(this, MainActivity::class.java)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
            )
        }
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
