package com.copypaste.app.quickaccess

import com.copypaste.app.JsonRuntimeAccess
import com.copypaste.app.RuntimeAccessWorker
import java.util.concurrent.Executor
import org.json.JSONObject

/** A synchronous runtime that records every request and answers from [answers] by method and op. */
internal class FakeQuickAccessRuntime(private val answers: (method: String, op: String?) -> JSONObject?) {
    val requests = mutableListOf<JSONObject>()
    val worker = RuntimeAccessWorker(
        JsonRuntimeAccess { raw -> answer(JSONObject(raw)) },
        Executor { it.run() },
        Executor { it.run() },
    )

    fun install(): FakeQuickAccessRuntime = also { QuickAccessRuntime.factory = { worker } }

    fun methods(): List<String> = requests.map { it.getString("method") }

    private fun answer(envelope: JSONObject): String {
        requests += envelope
        val method = envelope.getString("method")
        val op = envelope.optJSONObject("params")?.optJSONObject("op")?.optString("op")
        val data = answers(method, op) ?: JSONObject().put("empty", JSONObject())
        return JSONObject().put("id", 0).put("ok", true).put("data", data).toString()
    }
}
