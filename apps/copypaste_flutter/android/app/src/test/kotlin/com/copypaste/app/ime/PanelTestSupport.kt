package com.copypaste.app.ime

import android.content.Context
import android.content.Intent
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.InputConnection
import android.widget.TextView
import com.copypaste.app.JsonRuntimeAccess
import com.copypaste.app.RuntimeAccessWorker
import java.util.concurrent.Executor
import org.json.JSONArray
import org.json.JSONObject

/** A synchronous runtime that records every method and answers from [items]. */
internal class FakeRuntime(var items: List<JSONObject> = emptyList()) {
    val methods = mutableListOf<String>()
    val worker = RuntimeAccessWorker(
        JsonRuntimeAccess { raw -> answer(JSONObject(raw)) },
        Executor { it.run() },
        Executor { it.run() },
    )

    private fun answer(envelope: JSONObject): String {
        val method = envelope.getString("method")
        methods += method
        val params = envelope.optJSONObject("params")
        val data = when (method) {
            "history_query" -> JSONObject().put("page", JSONObject().put("items", JSONArray(items)).put("skipped_undecryptable", 0))
            "get" -> items.firstOrNull { it.getString("id") == params?.getString("id") }?.let { JSONObject().put("item", it) }
                ?: return """{"id":0,"ok":false,"error":"Not found.","error_code":"not_found"}"""
            "paste_queue" -> JSONObject().put("paste_queue", JSONObject().put("entries", JSONArray()))
            else -> JSONObject().put("empty", JSONObject())
        }
        return JSONObject().put("id", 0).put("ok", true).put("data", data).toString()
    }

    companion object {
        fun item(id: String, content: String, secret: Boolean = false, pinned: Boolean = false): JSONObject =
            JSONObject().put("id", id).put("content", content).put("content_type", "text/plain")
                .put("content_class", "text").put("created_at", 1_000L).put("pinned", pinned)
                .apply { if (secret) put("privacy", JSONObject().put("secret", true)) }
    }
}

internal class FakeClipboard(var stamp: Long? = null) : PanelClipboard {
    var allowed = true
    var background = false
    var captures = 0
    var listening = false
    var released = false
    var text: String? = null

    override fun captureAllowed(): Boolean = allowed
    override fun coveredByBackgroundCapture(): Boolean = background
    override fun clipStamp(): Long? = stamp
    override fun currentText(): String? = text
    override fun capture(onSaved: () -> Unit) { captures += 1 }
    override fun listen(onChanged: () -> Unit) { listening = true }
    override fun stopListening() { listening = false }
    override fun release() { released = true }
}

internal class FakeHost(var connection: InputConnection? = null) : PanelHost {
    val launched = mutableListOf<Intent>()
    var switchedBack = 0
    var secure = false

    override fun inputConnection(): InputConnection? = connection
    override fun switchBack() { switchedBack += 1 }
    override fun startActivity(intent: Intent) { launched += intent }
    override fun setSecureWindow(secure: Boolean) { this.secure = secure }
}

internal fun controller(
    context: Context,
    runtime: FakeRuntime,
    host: FakeHost = FakeHost(),
    clipboard: FakeClipboard = FakeClipboard(),
    locked: Boolean = false,
    unlock: SecretUnlock = SecretUnlock(),
): PanelController = PanelController(
    context,
    host,
    runtime.worker,
    clipboard,
    PanelPreferences(context),
    unlock = unlock,
    locked = { locked },
    authenticationAvailable = { true },
    screenshotsBlocked = { false },
)

/** Every text a view tree renders, including content descriptions. */
internal fun renderedText(view: View): List<String> {
    val texts = mutableListOf<String>()
    fun visit(node: View) {
        node.contentDescription?.let { texts += it.toString() }
        if (node is TextView) texts += node.text.toString()
        if (node is ViewGroup) for (index in 0 until node.childCount) visit(node.getChildAt(index))
    }
    visit(view)
    return texts
}
