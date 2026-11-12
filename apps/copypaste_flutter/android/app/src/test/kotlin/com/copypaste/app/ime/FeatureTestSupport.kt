package com.copypaste.app.ime

import android.content.Intent
import android.view.inputmethod.InputConnection
import com.copypaste.app.JsonRuntimeAccess
import com.copypaste.app.RuntimeAccessWorker
import java.util.concurrent.Executor
import org.json.JSONObject

/** A synchronous runtime for the feature tabs: [answer] maps a method and op to a reply. */
internal class FeatureRuntime(private val answer: (method: String, op: JSONObject) -> Answer) {
    sealed interface Answer {
        data class Ok(val kind: String, val value: Any) : Answer
        data class Fail(val code: String) : Answer
    }

    /** Every call as (method, op or params). */
    val calls = mutableListOf<Pair<String, JSONObject>>()
    val worker = RuntimeAccessWorker(
        JsonRuntimeAccess { raw -> respond(JSONObject(raw)) },
        Executor { it.run() },
        Executor { it.run() },
    )

    fun ops(method: String): List<JSONObject> = calls.filter { it.first == method }.map { it.second }

    fun op(method: String, name: String): JSONObject? = ops(method).lastOrNull { it.optString("op") == name }

    private fun respond(envelope: JSONObject): String {
        val method = envelope.getString("method")
        val params = envelope.optJSONObject("params") ?: JSONObject()
        val op = params.optJSONObject("op") ?: params
        calls += method to op
        return when (val reply = answer(method, op)) {
            is Answer.Ok -> JSONObject().put("id", 0).put("ok", true).put("data", JSONObject().put(reply.kind, reply.value))
            is Answer.Fail -> JSONObject().put("id", 0).put("ok", false).put("error", "refused").put("error_code", reply.code)
        }.toString()
    }

    companion object {
        val empty: Answer = Answer.Ok("empty", JSONObject())
    }
}

/** Records what a feature tab asked of the panel. */
internal class FakeTabHost(
    override var field: FieldPolicy,
    private val connection: () -> InputConnection?,
) : PanelTabHost {
    override val insert = PanelInsert(connection)
    val notices = mutableListOf<String>()
    var noticeAction: (() -> Unit)? = null
    val sheets = mutableListOf<Pair<String, List<PanelAction>>>()
    val outcomes = mutableListOf<InsertOutcome>()
    val announced = mutableListOf<String>()
    var unlocks = 0
    var clipboard: String? = null

    override fun inputConnection(): InputConnection? = connection()
    override fun clipboardText(): String? = clipboard
    override fun showActions(title: String, actions: List<PanelAction>) { sheets += title to actions }
    override fun showNotice(text: String, actionLabel: String?, action: (() -> Unit)?) {
        notices += text
        noticeAction = action
    }
    override fun announce(text: String) { announced += text }
    override fun unlockSecrets(action: () -> Unit) {
        unlocks += 1
        action()
    }
    override fun afterInsert(outcome: InsertOutcome, pasteViaClipboard: () -> Unit) { outcomes += outcome }
    override fun openKeypad(target: KeypadTarget) = Unit
    override fun closeKeypad() = Unit
    override fun selectTab(tab: PanelTab) = Unit
    override fun queueChanged() = Unit
    override fun startActivity(intent: Intent) = Unit
    override fun insertOriginal(item: PanelItem) = Unit
}
