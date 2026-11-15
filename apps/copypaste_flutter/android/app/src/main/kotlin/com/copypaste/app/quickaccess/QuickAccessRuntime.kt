package com.copypaste.app.quickaccess

import android.content.Context
import com.copypaste.app.RuntimeAccessWorker
import com.copypaste.app.RuntimeReply
import org.json.JSONObject

/** Runtime access for tiles and quick access activities; tests replace [factory] with a fake. */
internal object QuickAccessRuntime {
    @Volatile
    var factory: (Context) -> RuntimeAccessWorker = RuntimeAccessWorker::native

    fun worker(context: Context): RuntimeAccessWorker = factory(context)
}

/** Builds the `{"op": …}` envelope of a grouped feature method. */
internal fun op(name: String, fields: JSONObject.() -> Unit = {}): JSONObject =
    JSONObject().put("op", JSONObject().put("op", name).apply(fields))

/** `private_mode` of a `status` or `private_mode` reply; null when the reply has neither. */
internal fun pausedOf(reply: RuntimeReply): Boolean? {
    val data = (reply as? RuntimeReply.Ok)?.objectData() ?: return null
    return if (data.has("private_mode")) data.optBoolean("private_mode") else null
}
