package com.copypaste.app.quickaccess

import android.content.Context
import android.content.Intent
import android.net.Uri
import com.copypaste.app.AndroidClipboardReader
import com.copypaste.app.MainActivity
import com.copypaste.app.R
import com.copypaste.app.RuntimeAccessWorker
import com.copypaste.app.RuntimeReply
import org.json.JSONArray
import org.json.JSONObject

/** Content handed to "Add to Paste Queue" by text selection or the share sheet. */
internal sealed interface QueueIntakeContent {
    /** [via] is the wire `QueueTextVia`: `selection_action` or `share`. */
    data class Text(val text: String, val via: String) : QueueIntakeContent
    data class Image(val uri: Uri) : QueueIntakeContent
}

internal enum class QueueIntakeOutcome(val message: Int) {
    ADDED(R.string.quick_access_queue_added),
    NOTHING(R.string.quick_access_queue_nothing),
    FAILED(R.string.quick_access_queue_failed),
}

/**
 * Appends shared or selected content to the paste queue. Text goes in one
 * runtime call that stores it in history under the capture rules and
 * queues that clip; content the rules refuse is rejected, never queued
 * on its own. Nothing here reads the clipboard.
 */
internal object QueueIntake {
    const val VIA_SELECTION = "selection_action"
    const val VIA_SHARE = "share"

    fun parse(intent: Intent?): QueueIntakeContent? {
        when (intent?.action) {
            Intent.ACTION_PROCESS_TEXT -> {
                val text = intent.getCharSequenceExtra(Intent.EXTRA_PROCESS_TEXT)?.toString()
                return text?.takeIf(String::isNotBlank)?.let { QueueIntakeContent.Text(it, VIA_SELECTION) }
            }
            Intent.ACTION_SEND -> {
                val text = intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString()
                if (!text.isNullOrBlank()) return QueueIntakeContent.Text(text, VIA_SHARE)
                if (intent.type?.lowercase()?.startsWith("image/") != true) return null
                @Suppress("DEPRECATION")
                val uri = intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
                    ?: intent.clipData?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.uri
                return uri?.let(QueueIntakeContent::Image)
            }
            else -> return null
        }
    }

    fun addText(worker: RuntimeAccessWorker, content: QueueIntakeContent.Text, done: (QueueIntakeOutcome) -> Unit) {
        val source = JSONObject().put("kind", "text").put("text", content.text).put("via", content.via)
        add(worker, source, done)
    }

    /**
     * Stores the shared image through the explicit capture path (rules,
     * exclusions, size limits), then queues the clip it became. The intent
     * must stay with a live activity until [done]: its read grant ends with it.
     */
    fun addImage(context: Context, intent: Intent, worker: RuntimeAccessWorker, done: (QueueIntakeOutcome) -> Unit) {
        MainActivity.ensureRuntime(context.applicationContext)
        val host = AndroidClipboardReader.openHost(explicit = true) ?: return done(QueueIntakeOutcome.FAILED)
        AndroidClipboardReader.captureExplicit(context, intent, host) { captured ->
            if (!captured) return@captureExplicit done(QueueIntakeOutcome.FAILED)
            val query = JSONObject().put("content_classes", JSONArray().put("image")).put("sort", "newest")
            worker.request("history_query", JSONObject().put("query", query).put("limit", 1)) { reply ->
                val id = (reply as? RuntimeReply.Ok)?.objectData()?.optJSONArray("items")
                    ?.optJSONObject(0)?.optString("id")?.takeIf(String::isNotEmpty)
                    ?: return@request done(QueueIntakeOutcome.FAILED)
                add(worker, JSONObject().put("kind", "clip").put("id", id), done)
            }
        }
    }

    private fun add(worker: RuntimeAccessWorker, source: JSONObject, done: (QueueIntakeOutcome) -> Unit) {
        val params = op("add") { put("sources", JSONArray().put(source)) }
        worker.request("paste_queue", params) { reply ->
            done(if (reply is RuntimeReply.Ok) QueueIntakeOutcome.ADDED else QueueIntakeOutcome.FAILED)
        }
    }
}
